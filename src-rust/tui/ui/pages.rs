use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::tui::app::{App, proxy_actions};
use crate::tui::i18n;

use super::{content_block, display_width, highlight_symbol, render_key_bar_center, selection_style};

fn label_line<'a>(app: &App, label: &'a str, value: String) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {label}"),
            Style::default().fg(app.theme.accent),
        ),
        Span::styled(": ", Style::default().fg(app.theme.dim)),
        Span::raw(value),
    ])
}

pub(super) fn render_home(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = content_block(app, i18n::page_home());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let logo_height = 8u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(logo_height)])
        .split(inner);
    render_key_bar_center(frame, theme, chunks[0], &[("r", i18n::key_refresh())]);

    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Left: Profile & Daemon
    let daemon = if app.data.daemon.running {
        i18n::home_running(
            app.data.daemon.pid.unwrap_or(0),
            app.data.daemon.host.as_deref().unwrap_or("?"),
            app.data.daemon.port.unwrap_or(0),
        )
    } else {
        i18n::home_stopped().to_string()
    };
    let left_lines = vec![
        Line::default(),
        label_line(app, i18n::home_profiles(), app.data.profiles.len().to_string()),
        label_line(app, i18n::home_current(), app.data.config.current.clone().unwrap_or_else(|| "none".into())),
        label_line(app, i18n::home_write_mode(), app.data.config.settings.write_mode.clone()),
        label_line(app, i18n::home_proxy_daemon(), daemon),
        Line::default(),
        label_line(app, i18n::home_requests(), i18n::home_requests_fmt(
            app.data.stats.total_requests, app.data.stats.ok_requests, &app.data.stats.success_rate,
        )),
    ];
    let left_block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Plain)
        .border_style(Style::default().fg(theme.dim))
        .title("Overview");
    let left_inner = left_block.inner(sections[0]);
    frame.render_widget(left_block, sections[0]);
    frame.render_widget(Paragraph::new(left_lines), left_inner);

    // Right: Paths
    let right_lines = vec![
        Line::default(),
        label_line(app, i18n::home_config(), crate::config::config_path().display().to_string()),
        label_line(app, i18n::home_pi_models(), crate::config::models_path().display().to_string()),
        label_line(app, i18n::home_backups(), crate::config::backup_dir().display().to_string()),
    ];
    let right_block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Plain)
        .border_style(Style::default().fg(theme.dim))
        .title("Paths");
    let right_inner = right_block.inner(sections[1]);
    frame.render_widget(right_block, sections[1]);
    frame.render_widget(Paragraph::new(right_lines), right_inner);

    // Bottom: ASCII logo
    let logo_lines: Vec<Line> = i18n::home_logo()
        .lines()
        .map(|s| Line::from(Span::styled(s.to_string(), Style::default().fg(theme.surface))))
        .collect();
    let tagline = Line::from(Span::styled(
        i18n::home_tagline(),
        Style::default().fg(theme.dim),
    ));
    let mut bottom_lines = logo_lines;
    bottom_lines.push(Line::default());
    bottom_lines.push(tagline);
    frame.render_widget(
        Paragraph::new(bottom_lines).alignment(ratatui::layout::Alignment::Center),
        chunks[2],
    );
}

pub(super) fn render_proxy(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = content_block(app, i18n::page_proxy());
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
        &[("↑↓", i18n::key_move()), ("Enter", i18n::key_run_action()), ("Esc", i18n::key_back())],
    );

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(chunks[1]);

    // Actions block
    let items: Vec<ListItem<'_>> = proxy_actions()
        .iter()
        .map(|action| ListItem::new(format!("  {action}")))
        .collect();
    let list = List::new(items)
        .highlight_style(selection_style(theme))
        .highlight_symbol(super::highlight_symbol(theme))
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title("Actions"),
        );
    let mut state = ListState::default();
    state.select(Some(app.proxy_idx));
    frame.render_stateful_widget(list, sections[0], &mut state);

    // Status block
    let proxy = &app.data.config.settings.proxy;
    let mut status_lines = vec![Line::default()];
    if app.data.daemon.running {
        status_lines.push(Line::from(Span::styled(
            i18n::proxy_running(app.data.daemon.pid.unwrap_or(0)),
            Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
        )));
    } else {
        status_lines.push(Line::from(Span::styled(
            i18n::proxy_stopped(),
            Style::default().fg(theme.warn),
        )));
    }
    status_lines.push(Line::default());
    status_lines.push(label_line(app, i18n::proxy_listen(), format!("{}:{}", proxy.host, proxy.port)));
    status_lines.push(label_line(app, i18n::proxy_target(), proxy.target.clone().unwrap_or_else(|| "—".into())));
    status_lines.push(label_line(
        app,
        i18n::proxy_failover(),
        if proxy.failover.is_empty() { "—".into() } else { proxy.failover.join(" → ") },
    ));
    status_lines.push(Line::default());
    status_lines.push(Line::from(Span::styled(
        format!("  {}", app.data.daemon.message),
        Style::default().fg(theme.dim),
    )));

    let status_block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Plain)
        .border_style(Style::default().fg(theme.dim))
        .title("Status");
    let status_inner = status_block.inner(sections[1]);
    frame.render_widget(status_block, sections[1]);
    frame.render_widget(Paragraph::new(status_lines).wrap(Wrap { trim: false }), status_inner);
}

pub(super) fn render_stats(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = content_block(app, i18n::page_stats());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    render_key_bar_center(
        frame,
        &app.theme,
        chunks[0],
        &[("r", i18n::key_refresh()), ("Esc", i18n::key_back())],
    );

    let stats = &app.data.stats;

    if stats.total_requests == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                i18n::stats_no_data(),
                Style::default().fg(app.theme.dim),
            ))),
            chunks[1],
        );
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(chunks[1]);

    // Overview block
    let mut overview_lines: Vec<Line<'_>> = vec![Line::default()];
    overview_lines.push(label_line(
        app,
        i18n::home_requests(),
        i18n::stats_requests_fmt(
            stats.total_requests,
            stats.ok_requests,
            stats.failed_requests,
            &stats.success_rate,
        ),
    ));
    if stats.avg_latency_ms > 0 {
        overview_lines.push(label_line(
            app,
            i18n::stats_avg_latency(),
            format!("{}ms", stats.avg_latency_ms),
        ));
    }
    if stats.retried_requests > 0 || stats.skipped_by_circuit > 0 {
        overview_lines.push(label_line(
            app,
            i18n::stats_retries_skipped(),
            format!("{} / {}", stats.retried_requests, stats.skipped_by_circuit),
        ));
    }

    let overview_block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Plain)
        .border_style(Style::default().fg(app.theme.dim))
        .title("Overview");
    let overview_inner = overview_block.inner(sections[0]);
    frame.render_widget(overview_block, sections[0]);
    frame.render_widget(Paragraph::new(overview_lines), overview_inner);

    // Details block
    let mut detail_lines: Vec<Line<'_>> = vec![Line::default()];
    if !stats.by_provider.is_empty() {
        detail_lines.push(Line::from(Span::styled(
            i18n::stats_by_provider(),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        let mut providers: Vec<_> = stats.by_provider.iter().collect();
        providers.sort_by(|a, b| b.1.total.cmp(&a.1.total));
        for (name, ps) in providers {
            let rate = if ps.total > 0 {
                format!("{:.0}%", (ps.ok as f64 / ps.total as f64) * 100.0)
            } else {
                "0%".into()
            };
            detail_lines.push(Line::from(format!(
                "    {}: {} req, {} ok ({}), avg {}ms",
                name, ps.total, ps.ok, rate, ps.avg_ms
            )));
        }
    }
    if !stats.by_model.is_empty() {
        detail_lines.push(Line::default());
        detail_lines.push(Line::from(Span::styled(
            i18n::stats_by_model(),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        let mut models: Vec<_> = stats.by_model.iter().collect();
        models.sort_by(|a, b| b.1.total.cmp(&a.1.total));
        for (name, ms) in models {
            detail_lines.push(Line::from(format!(
                "    {}: {} req, {} ok",
                name, ms.total, ms.ok
            )));
        }
    }

    let details_block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Plain)
        .border_style(Style::default().fg(app.theme.dim))
        .title("Details");
    let details_inner = details_block.inner(sections[1]);
    frame.render_widget(details_block, sections[1]);
    frame.render_widget(Paragraph::new(detail_lines).wrap(Wrap { trim: false }), details_inner);
}

pub(super) fn render_backups(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = content_block(app, i18n::page_backups_count(app.data.backups.len()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    render_key_bar_center(
        frame,
        &app.theme,
        chunks[0],
        &[("↑↓", i18n::key_move()), ("Esc", i18n::key_back())],
    );

    if app.data.backups.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                i18n::backups_empty(),
                Style::default().fg(app.theme.dim),
            ))),
            chunks[1],
        );
        return;
    }

    let items: Vec<ListItem<'_>> = app
        .data
        .backups
        .iter()
        .map(|name| ListItem::new(format!("  {name}")))
        .collect();
    let list = List::new(items)
        .highlight_style(selection_style(&app.theme))
        .highlight_symbol(super::highlight_symbol(&app.theme));

    let mut state = ListState::default();
    state.select(Some(app.backups_idx));
    frame.render_stateful_widget(list, chunks[1], &mut state);
}

pub(super) fn render_settings(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = content_block(app, i18n::page_settings());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let editing = app.settings_editing_field.is_some();
    let key_hints: &[(&str, &str)] = if editing {
        &[("Esc", i18n::key_close()), ("Enter", i18n::key_save())]
    } else {
        &[
            ("↑↓", i18n::key_move()),
            ("←→/Space", if app.settings_proxy_idx == 0 { i18n::key_switch() } else { "" }),
            ("Enter", i18n::key_edit()),
            ("Esc", i18n::key_back()),
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    render_key_bar_center(frame, theme, chunks[0], key_hints);

    let proxy = &app.data.config.settings.proxy;
    let failover_str = if proxy.failover.is_empty() {
        "—".to_string()
    } else {
        proxy.failover.join(", ")
    };

    let rows_data: Vec<(&str, String)> = vec![
        (i18n::settings_lang_label(), if app.settings_lang_idx == 0 {
            i18n::settings_lang_en().to_string()
        } else {
            i18n::settings_lang_zh().to_string()
        }),
        (i18n::settings_proxy_host(), proxy.host.clone()),
        (i18n::settings_proxy_port(), proxy.port.to_string()),
        (i18n::settings_proxy_target(), proxy.target.clone().unwrap_or_else(|| "—".into())),
        (i18n::settings_proxy_failover(), failover_str),
    ];

    let label_width = rows_data
        .iter()
        .map(|(label, _)| display_width(label))
        .max()
        .unwrap_or(8)
        + 4;

    let header = Row::new(vec![
        Cell::from(Span::styled(
            i18n::settings_header_setting(),
            Style::default().fg(theme.dim).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            i18n::settings_header_value(),
            Style::default().fg(theme.dim).add_modifier(Modifier::BOLD),
        )),
    ]);

    let rows: Vec<Row> = rows_data
        .iter()
        .enumerate()
        .map(|(i, (label, value))| {
            let mut display_value = value.clone();
            if app.settings_editing_field == Some(i) {
                display_value = format!("{}▎", app.settings_edit_input.value);
            }
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("  {label}"),
                    Style::default().fg(theme.accent),
                )),
                Cell::from(display_value),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Length(label_width), Constraint::Min(10)],
    )
    .header(header)
    .column_spacing(2)
    .row_highlight_style(selection_style(theme))
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.settings_proxy_idx));
    frame.render_stateful_widget(table, chunks[1], &mut state);

    // Cursor for editing
    if let Some(_field_idx) = app.settings_editing_field {
        if let Some(cell) = state.selected() {
            let y = chunks[1].y + 1 + cell as u16; // +1 for header
            let x = chunks[1].x + label_width + 2;
            let (_, cursor_x) = super::super::text_edit::visible_text_window(
                &app.settings_edit_input.value,
                app.settings_edit_input.cursor,
                chunks[1].width.saturating_sub(label_width).saturating_sub(2).max(1),
            );
            frame.set_cursor_position((x + cursor_x, y));
        }
    }
}
