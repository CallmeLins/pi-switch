use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::tui::app::App;
use crate::tui::form::{FieldKind, FormFocus, FormMode, API_CHOICES};
use crate::tui::i18n;
use crate::tui::text_edit::visible_text_window;

use super::{
    content_block, display_width, highlight_symbol, render_key_bar_center, selection_style,
};

// ─── Profiles table ───────────────────────────────────────

pub(super) fn render_profiles(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let visible = app.visible_profiles();
    let total = app.data.profiles.len();
    let title = if visible.len() == total {
        i18n::page_profiles(total)
    } else {
        i18n::page_profiles_filtered(visible.len(), total)
    };
    let block = content_block(app, title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let show_filter = app.filter_active || !app.filter.value.is_empty();
    let constraints = if show_filter {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ]
    } else {
        vec![Constraint::Length(1), Constraint::Min(0)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let add = i18n::key_add(); let fltr = i18n::key_filter();
    let empty_keys: &[(&str, &str)] = &[("a", add), ("/", fltr)];
    let detail = i18n::key_detail(); let switch = i18n::key_switch();
    let copy = i18n::key_copy(); let edit = i18n::key_edit(); let del = i18n::key_delete();
    let full_keys: &[(&str, &str)] = &[
        ("Enter", detail),
        ("Space", switch),
        ("a", add),
        ("c", copy),
        ("e", edit),
        ("d", del),
        ("/", fltr),
    ];
    render_key_bar_center(
        frame,
        theme,
        chunks[0],
        if total == 0 { empty_keys } else { full_keys },
    );

    let table_area = if show_filter {
        render_filter_line(frame, app, chunks[1]);
        chunks[2]
    } else {
        chunks[1]
    };

    if visible.is_empty() {
        let message = if total == 0 {
            i18n::profiles_empty_add()
        } else {
            i18n::profiles_no_match()
        };
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.dim))
                .wrap(Wrap { trim: false }),
            vertically_centered(table_area, 5),
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(i18n::profiles_name_id_col()),
        Cell::from(i18n::profiles_api_url_col()),
    ])
    .style(Style::default().fg(theme.dim).add_modifier(Modifier::BOLD));

    let rows = visible.iter().map(|row| {
        // Marker: * for in failover chain, space otherwise
        let marker = if row.in_failover_chain {
            " * "
        } else {
            "   "
        };

        // Row color: red if circuit breaker is open (any profile),
        // green if in failover chain and healthy, default otherwise.
        let row_color = if row.circuit_breaker_open {
            Some(theme.err)
        } else if row.in_failover_chain {
            Some(theme.ok)
        } else {
            None
        };

        // Priority label — position in failover chain
        let priority_label = if let Some(priority) = row.failover_priority {
            format!(" [p{}]", priority)
        } else {
            String::new()
        };

        // Name cell: name + priority label + optional proxy badge
        let mut name_spans = vec![Span::raw(row.name.clone())];

        // Add priority label
        if !priority_label.is_empty() {
            name_spans.push(Span::raw(priority_label));
        }

        // Add proxy badge
        if row.proxy {
            name_spans.push(Span::raw(" "));
            name_spans.push(Span::styled(
                i18n::profiles_proxy_badge(),
                if theme.no_color {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
                },
            ));
        }

        // Circuit breaker indicator — show status code (e.g. "502") instead of icon
        if row.circuit_breaker_open {
            name_spans.push(Span::raw(" "));
            let code = row.circuit_breaker_error
                .as_deref()
                .and_then(|e| {
                    // Extract status code from "HTTP 502" or "status 503" etc.
                    e.split_whitespace()
                        .find(|w| w.chars().all(|c| c.is_ascii_digit()))
                })
                .unwrap_or("ERR");
            name_spans.push(Span::styled(
                code,
                Style::default().fg(theme.err).add_modifier(Modifier::BOLD),
            ));
        }

        // Add exposed models count as sub-line
        if row.exposed_count > 0 {
            name_spans.push(Span::raw("\n"));
            name_spans.push(Span::styled(
                format!("  {} exposed", row.exposed_count),
                Style::default().fg(theme.dim),
            ));
        }

        let name_cell = Line::from(name_spans);

        // API URL cell: base_url + provider ID sub-line
        let api_cell = vec![
            Span::raw(row.base_url.clone()),
            Span::raw("\n"),
            Span::styled(
                format!("  {}", row.provider_id),
                Style::default().fg(theme.dim),
            ),
        ];

        let mut table_row = Row::new(vec![
            Cell::from(Span::raw(format!(" {marker}"))),
            Cell::from(name_cell),
            Cell::from(Line::from(api_cell)),
        ]);

        // Apply row color if in failover chain
        if let Some(color) = row_color {
            table_row = table_row.style(Style::default().fg(color));
        }

        table_row
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Percentage(43),
            Constraint::Percentage(45),
        ],
    )
    .header(header)
    .row_highlight_style(selection_style(theme))
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.profiles_idx.min(visible.len().saturating_sub(1))));
    frame.render_stateful_widget(table, table_area, &mut state);
}

fn render_filter_line(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let prefix = " / ";
    let prefix_width = display_width(prefix);
    let input_width = area.width.saturating_sub(prefix_width).max(1);
    let (visible, cursor_x) =
        visible_text_window(&app.filter.value, app.filter.cursor, input_width);

    let line = Line::from(vec![
        Span::styled(
            prefix,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(visible),
    ]);
    frame.render_widget(Paragraph::new(line), area);

    if app.filter_active {
        frame.set_cursor_position((area.x + prefix_width + cursor_x, area.y));
    }
}

fn vertically_centered(area: Rect, content_height: u16) -> Rect {
    let height = content_height.min(area.height);
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(area.x, y, area.width, height)
}

// ─── Profile detail ───────────────────────────────────────

fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 12 || key.starts_with('$') {
        key.to_string()
    } else {
        let head: String = chars[..6].iter().collect();
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("{head}…{tail}")
    }
}

pub(super) fn render_profile_detail(frame: &mut Frame<'_>, app: &App, area: Rect, name: &str) {
    let theme = &app.theme;
    let block = content_block(app, i18n::detail_title(name));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    render_key_bar_center(
        frame,
        theme,
        chunks[0],
        &[
            ("e", i18n::key_edit()),
            ("Space", i18n::key_switch()),
            ("d", i18n::key_delete()),
            ("x", i18n::key_expose()),
            ("u", if i18n::is_zh() { "用户代理" } else { "UA" }),
            ("↑↓", i18n::key_scroll()),
            ("Esc", i18n::key_back()),
        ],
    );

    let Some(value) = app.data.config.profiles.get(name) else {
        frame.render_widget(
            Paragraph::new(i18n::detail_not_found()).style(Style::default().fg(theme.err)),
            chunks[1],
        );
        return;
    };

    let label = |label: &str, value: String| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("  {label}"),
                Style::default().fg(theme.accent),
            ),
            Span::styled(": ", Style::default().fg(theme.dim)),
            Span::raw(value),
        ])
    };

    let get_str =
        |key: &str| -> String { value.get(key).and_then(|v| v.as_str()).unwrap_or("").into() };

    let is_current = app.data.config.current.as_deref() == Some(name);
    let mut lines = vec![
        Line::default(),
        label(i18n::detail_name(), name.to_string()),
        label(
            i18n::detail_current(),
            if is_current { i18n::detail_current_yes().into() } else { i18n::detail_current_no().into() },
        ),
        label(
            i18n::detail_provider_id(),
            crate::config::provider_id_for(&app.data.config, name),
        ),
        label(i18n::detail_api(), get_str("api")),
        label(i18n::detail_base_url(), get_str("baseUrl")),
        label(i18n::detail_api_key(), mask_key(&get_str("apiKey"))),
    ];

    if let Some(preset) = value.get("preset").and_then(|v| v.as_str()) {
        lines.push(label(i18n::detail_preset(), preset.to_string()));
    }
    let spoof_val = value.get("userAgent").and_then(|v| v.as_str());
    lines.push(label(
        if i18n::is_zh() { "用户代理" } else { "User-Agent" },
        spoof_val.unwrap_or(if i18n::is_zh() { "（用全局）" } else { "(global)" }).to_string(),
    ));
    if let Some(updated) = value.get("updatedAt").and_then(|v| v.as_str()) {
        lines.push(label(i18n::detail_updated(), updated.to_string()));
    }

    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        i18n::detail_models(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    if let Some(models) = value.get("models").and_then(|v| v.as_array()) {
        for model in models {
            let id = model.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let name = model.get("name").and_then(|v| v.as_str());
            let ctx = model
                .get("context_window")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let max = model
                .get("max_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let mut text = format!("    • {id}");
            if let Some(name) = name {
                text.push_str(&format!("  ({name})"));
            }
            text.push_str(&format!("  ctx {ctx} / max {max}"));
            lines.push(Line::from(text));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .scroll((app.detail_scroll, 0))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
}

// ─── Provider form ────────────────────────────────────────

pub(super) fn render_form(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some(form) = &app.form else {
        return;
    };

    let title = match &form.mode {
        FormMode::Add => i18n::form_add_title().to_string(),
        FormMode::Edit(name) => i18n::form_edit_title(name),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme.dim))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let is_add = matches!(form.mode, FormMode::Add);
    let constraints = if is_add {
        vec![
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ]
    } else {
        vec![Constraint::Length(1), Constraint::Min(0)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    render_key_bar_center(
        frame,
        theme,
        chunks[0],
        &[
            ("Tab", i18n::key_focus()),
            ("↑↓", i18n::key_field()),
            ("Enter", i18n::key_edit_apply()),
            ("Ctrl+S", i18n::key_save()),
            ("Esc", i18n::key_close()),
        ],
    );

    let body_area = if is_add {
        render_template_chips(frame, app, chunks[1]);
        chunks[2]
    } else {
        chunks[1]
    };

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(body_area);

    render_form_fields(frame, app, panes[0]);
    render_json_preview(frame, app, panes[1]);
}

fn active_chip_style(theme: &crate::tui::theme::Theme) -> Style {
    if theme.no_color {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    }
}

fn inactive_chip_style(theme: &crate::tui::theme::Theme) -> Style {
    if theme.no_color {
        Style::default()
    } else {
        Style::default().fg(Color::White).bg(theme.surface)
    }
}

fn render_template_chips(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some(form) = &app.form else {
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(super::pane_border_style(
            theme,
            form.focus == FormFocus::Templates,
        ))
        .title(i18n::form_template_pane());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans: Vec<Span<'_>> = vec![Span::raw(" ")];
    let mut labels: Vec<&str> = vec![i18n::form_custom_chip()];
    labels.extend(app.data.presets.iter().map(|p| p.name.as_str()));

    for (idx, label) in labels.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" "));
        }
        let style = if idx == form.template_idx {
            active_chip_style(theme)
        } else {
            inactive_chip_style(theme)
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn render_form_fields(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some(form) = &app.form else {
        return;
    };
    let fields_focused = form.focus == FormFocus::Fields;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(super::pane_border_style(theme, fields_focused))
        .title(i18n::form_fields_pane());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)])
        .split(inner);

    // Key bar at top
    super::render_key_bar_center(
        frame,
        theme,
        chunks[0],
        &[
            ("f", i18n::key_fetch()),
            ("d", i18n::key_delete()),
            ("Enter", i18n::key_edit()),
        ],
    );

    let label_width = FieldKind::ALL
        .iter()
        .map(|f| display_width(f.label()))
        .max()
        .unwrap_or(8)
        + 2;

    let rows = FieldKind::ALL.iter().map(|field| {
        let value = match field {
            FieldKind::Api => {
                format!("◂ {} ▸", API_CHOICES[form.api_idx])
            }
            _ => form.field_value(*field),
        };
        Row::new(vec![
            Cell::from(Span::styled(
                format!(" {}", field.label()),
                Style::default().fg(theme.accent),
            )),
            Cell::from(value),
        ])
    });

    let table = Table::new(
        rows,
        [Constraint::Length(label_width), Constraint::Min(0)],
    )
    .column_spacing(1)
    .row_highlight_style(if fields_focused {
        selection_style(theme)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    })
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(form.field_idx));
    frame.render_stateful_widget(table, chunks[1], &mut state);

    // Edit box for current field
    let field = form.current_field();
    let editing = form.editing;
    let edit_border = if editing {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dim)
    };
    let edit_title = if editing {
        i18n::form_editing(field.label())
    } else {
        i18n::form_field_title(field.label())
    };
    let edit_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(edit_border)
        .title(edit_title);
    let edit_inner = edit_block.inner(chunks[2]);
    frame.render_widget(edit_block, chunks[2]);

    match field {
        FieldKind::Api => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(format!(" {}", API_CHOICES[form.api_idx])),
                    Span::styled(
                        i18n::form_api_cycle_hint(),
                        Style::default().fg(theme.dim),
                    ),
                ])),
                edit_inner,
            );
        }
        FieldKind::Models => {
            let input_width = edit_inner.width.saturating_sub(1).max(1);
            let (visible, cursor_x) = visible_text_window(&form.models.value, form.models.cursor, input_width);
            frame.render_widget(
                Paragraph::new(format!(" {visible}")),
                edit_inner,
            );
            if editing {
                frame.set_cursor_position((edit_inner.x + 1 + cursor_x, edit_inner.y));
            }
        }
        _ => {
            let input = match field {
                FieldKind::Name => &form.name,
                FieldKind::BaseUrl => &form.base_url,
                FieldKind::ApiKey => &form.api_key,
                FieldKind::Models => unreachable!(),
                FieldKind::Api => unreachable!(),
            };
            let input_width = edit_inner.width.saturating_sub(1).max(1);
            let (visible, cursor_x) = visible_text_window(&input.value, input.cursor, input_width);
            frame.render_widget(Paragraph::new(format!(" {visible}")), edit_inner);
            if editing {
                frame.set_cursor_position((edit_inner.x + 1 + cursor_x, edit_inner.y));
            }
        }
    }
}

fn render_json_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some(form) = &app.form else {
        return;
    };

    let editing = form.json_editing;
    let title = if editing {
        i18n::form_json_editing_pane()
    } else {
        i18n::form_json_preview_pane()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(super::pane_border_style(
            theme,
            form.focus == FormFocus::JsonPreview || editing,
        ))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if editing {
        format!(" {}▎", form.json_edit.value)
    } else {
        form.json_preview()
    };

    frame.render_widget(
        Paragraph::new(text)
            .style(if editing {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.cyan)
            })
            .scroll((form.json_scroll, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );

    if editing {
        // Calculate cursor position in multi-line text
        let lines: Vec<&str> = form.json_edit.value.lines().collect();
        let mut cursor_pos = 0;
        let mut cursor_line = 0;
        let mut cursor_col = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_len = line.chars().count();
            if cursor_pos + line_len >= form.json_edit.cursor {
                cursor_line = line_idx;
                cursor_col = form.json_edit.cursor - cursor_pos;
                break;
            }
            cursor_pos += line_len + 1; // +1 for newline
        }

        // Adjust cursor position relative to scroll
        let visible_line = cursor_line.saturating_sub(form.json_scroll as usize);
        let cursor_y = inner.y + visible_line as u16;
        let cursor_x = inner.x + 1 + cursor_col as u16;

        // Only set cursor if within visible area
        if cursor_y >= inner.y && cursor_y < inner.y + inner.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

// ─── Model Selection ──────────────────────────────────────

pub fn render_model_selection(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    provider_name: &str,
    is_expose_mode: bool,
) {
    let title = if is_expose_mode {
        i18n::model_selection_title_expose(provider_name)
    } else {
        i18n::model_selection_title_fetch(provider_name)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(super::pane_border_style(
            &app.theme,
            app.focus == crate::tui::app::Focus::Content,
        ))
        .title(Span::raw(title));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.model_selection_loading {
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner = spinner_chars[(app.tick % 10) as usize];
        let loading_text = format!("{} {}", spinner, i18n::model_selection_loading());
        let loading = Paragraph::new(loading_text)
            .style(Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))
            .alignment(ratatui::layout::Alignment::Center);

        // Center vertically
        let center_y = area.height / 2;
        let loading_area = Rect::new(
            inner.x,
            inner.y + center_y.saturating_sub(1),
            inner.width,
            3,
        );
        frame.render_widget(loading, loading_area);
        return;
    }

    if app.model_selection_list.is_empty() {
        let empty_msg = if is_expose_mode {
            i18n::model_selection_empty_expose()
        } else {
            i18n::model_selection_empty_fetch()
        };
        let empty = Paragraph::new(empty_msg)
            .style(Style::default().fg(app.theme.dim));
        frame.render_widget(empty, inner);
        return;
    }

    // Split area: list on left, help on right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner);

    // Render checklist
    let items: Vec<ListItem> = app.model_selection_list
        .iter()
        .enumerate()
        .map(|(idx, (model_id, selected))| {
            let checkbox = if *selected { "[√]" } else { "[ ]" };
            let text = format!(" {} {}", checkbox, model_id);

            let style = if idx == app.model_selection_idx {
                selection_style(&app.theme)
            } else {
                Style::default()
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items)
        .highlight_symbol(highlight_symbol(&app.theme))
        .highlight_style(selection_style(&app.theme));

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.model_selection_idx));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    // Render help
    let help_text = if is_expose_mode {
        vec![
            Line::from(i18n::model_selection_help_expose_title()),
            Line::from(""),
            Line::from(vec![
                Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(i18n::model_selection_help_toggle_text()),
            ]),
            Line::from(vec![
                Span::styled("s", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(i18n::model_selection_help_save_expose()),
            ]),
            Line::from(vec![
                Span::styled("q/Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(i18n::model_selection_help_cancel_text()),
            ]),
            Line::from(""),
            Line::from(i18n::model_selection_help_expose_desc1()),
            Line::from(i18n::model_selection_help_expose_desc2()),
        ]
    } else {
        vec![
            Line::from(i18n::model_selection_help_fetch_title()),
            Line::from(""),
            Line::from(vec![
                Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(i18n::model_selection_help_toggle_text()),
            ]),
            Line::from(vec![
                Span::styled("s", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(i18n::model_selection_help_save_models()),
            ]),
            Line::from(vec![
                Span::styled("q/Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(i18n::model_selection_help_cancel_text()),
            ]),
            Line::from(""),
            Line::from(i18n::model_selection_help_fetch_desc1()),
            Line::from(i18n::model_selection_help_fetch_desc2()),
        ]
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(app.theme.dim))
        .wrap(Wrap { trim: false });

    frame.render_widget(help, chunks[1]);
}
