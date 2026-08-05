use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::stats::TokenTotals;
use crate::tui::app::{proxy_actions, App};
use crate::tui::i18n;

use super::{
    content_block, display_width, highlight_symbol, render_key_bar_center, selection_style,
};

fn label_line<'a>(app: &App, label: &'a str, value: String) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {label}"), Style::default().fg(app.theme.accent)),
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
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(logo_height),
        ])
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
        label_line(
            app,
            i18n::home_profiles(),
            app.data.profiles.len().to_string(),
        ),
        label_line(
            app,
            i18n::home_current(),
            app.data
                .config
                .current
                .clone()
                .unwrap_or_else(|| "none".into()),
        ),
        label_line(
            app,
            i18n::home_write_mode(),
            app.data.config.settings.write_mode.clone(),
        ),
        label_line(app, i18n::home_proxy_daemon(), daemon),
        Line::default(),
        label_line(
            app,
            i18n::home_requests(),
            i18n::home_requests_fmt(
                app.data.stats.total_requests,
                app.data.stats.ok_requests,
                &app.data.stats.success_rate,
            ),
        ),
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
        label_line(
            app,
            i18n::home_config(),
            crate::config::config_path().display().to_string(),
        ),
        label_line(
            app,
            i18n::home_pi_models(),
            crate::config::models_path().display().to_string(),
        ),
        label_line(
            app,
            i18n::home_backups(),
            crate::config::backup_dir().display().to_string(),
        ),
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
        .map(|s| {
            Line::from(Span::styled(
                s.to_string(),
                Style::default().fg(theme.surface),
            ))
        })
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
        &[
            ("↑↓", i18n::key_move()),
            ("Enter", i18n::key_run_action()),
            ("Esc", i18n::key_back()),
        ],
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
    status_lines.push(label_line(
        app,
        i18n::proxy_listen(),
        format!("{}:{}", proxy.host, proxy.port),
    ));

    // Gateway mode: pi sees one provider that advertises every exposed model as
    // "profile/model"; the proxy routes by the model name in each request — no single target.
    let exposed_total: usize = app.data.profiles.iter().map(|p| p.exposed_count).sum();
    let with_models = app
        .data
        .profiles
        .iter()
        .filter(|p| p.exposed_count > 0)
        .count();
    status_lines.push(label_line(
        app,
        if i18n::is_zh() { "网关" } else { "Gateway" },
        app.data.config.settings.provider_prefix.clone(),
    ));
    status_lines.push(label_line(
        app,
        if i18n::is_zh() {
            "暴露模型"
        } else {
            "Models"
        },
        format!(
            "{} ({} {})",
            exposed_total,
            with_models,
            if i18n::is_zh() {
                "个供应商"
            } else {
                "providers"
            }
        ),
    ));
    // Informational: the model pi currently has selected.
    if let Some(ref model) = app.data.pi_default_model {
        status_lines.push(label_line(
            app,
            if i18n::is_zh() {
                "Pi 当前模型"
            } else {
                "Pi model"
            },
            model.clone(),
        ));
    }

    status_lines.push(label_line(
        app,
        i18n::proxy_failover(),
        if proxy.failover.is_empty() {
            "—".into()
        } else {
            proxy.failover.join(" → ")
        },
    ));

    // Provider health: one line per profile with exposed models, color-coded by circuit breaker state
    status_lines.push(Line::default());
    status_lines.push(Line::from(Span::styled(
        if i18n::is_zh() {
            "  供应商健康状态:"
        } else {
            "  Provider Health:"
        },
        Style::default().fg(theme.accent),
    )));
    let mut any_profile = false;
    for row in &app.data.profiles {
        if row.exposed_count == 0 || row.proxy {
            continue;
        }
        any_profile = true;
        let dot_color = if row.circuit_breaker_open {
            theme.err
        } else {
            theme.ok
        };
        let status_text: String = if row.circuit_breaker_open {
            row.circuit_breaker_error
                .as_deref()
                .and_then(|e| {
                    e.split_whitespace()
                        .find(|w| w.chars().all(|c| c.is_ascii_digit()))
                })
                .unwrap_or("ERR")
                .to_string()
        } else {
            "OK".to_string()
        };
        let mut spans = vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                format!("{} ", status_text),
                Style::default().fg(dot_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{} ", row.name)),
        ];
        if row.in_failover_chain {
            if let Some(p) = row.failover_priority {
                spans.push(Span::styled(
                    format!("[p{}]", p),
                    Style::default().fg(theme.dim),
                ));
            }
        }
        spans.push(Span::styled(
            format!(" ({} models)", row.exposed_count),
            Style::default().fg(theme.dim),
        ));
        status_lines.push(Line::from(spans));
    }
    if !any_profile {
        status_lines.push(Line::from(Span::styled(
            if i18n::is_zh() {
                "    (无已暴露模型的供应商)"
            } else {
                "    (no profiles with exposed models)"
            },
            Style::default().fg(theme.dim),
        )));
    }
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
    frame.render_widget(
        Paragraph::new(status_lines).wrap(Wrap { trim: false }),
        status_inner,
    );
}

pub(super) fn render_ccswitch_import(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = content_block(
        app,
        if i18n::is_zh() {
            "从 cc-switch 导入 Provider"
        } else {
            "Import Providers from cc-switch"
        },
    );
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
            ("↑↓", i18n::key_move()),
            ("Space", if i18n::is_zh() { "勾选" } else { "Toggle" }),
            ("Enter/s", i18n::key_save()),
            ("Esc", i18n::key_back()),
        ],
    );

    if let Some(err) = &app.ccswitch_error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                err.clone(),
                Style::default().fg(theme.err),
            )))
            .wrap(Wrap { trim: false }),
            chunks[1],
        );
        return;
    }

    if app.ccswitch_list.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                if i18n::is_zh() {
                    "cc-switch 中没有可导入的 provider"
                } else {
                    "No importable providers in cc-switch"
                },
                Style::default().fg(theme.dim),
            ))),
            chunks[1],
        );
        return;
    }

    let items: Vec<ListItem<'_>> = app
        .ccswitch_list
        .iter()
        .map(|(p, selected)| {
            let checkbox = if *selected { "[✓]" } else { "[ ]" };
            let exists_tag = if p.exists { " (已存在)" } else { "" };
            let api_label = match p.api.as_str() {
                "anthropic-messages" => "anthropic",
                "openai-responses" => "openai",
                "google-generative-ai" => "gemini",
                _ => p.api.as_str(),
            };
            let models = p.models.join(", ");
            let text = format!(
                "  {} {} [{}{}] {}  ·  {}",
                checkbox,
                p.name,
                api_label,
                exists_tag,
                if models.is_empty() { "-" } else { &models },
                p.base_url,
            );
            ListItem::new(text)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(selection_style(theme))
        .highlight_symbol(highlight_symbol(theme));

    let mut state = ListState::default();
    state.select(Some(app.ccswitch_idx));
    frame.render_stateful_widget(list, chunks[1], &mut state);
}

pub(super) fn render_packages(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = content_block(app, "Packages");
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
            ("↑↓/jk", "Move"),
            ("Space", "Toggle"),
            ("d", "Uninstall"),
            ("i", "Import"),
            ("r", "Refresh"),
            ("Esc", i18n::key_back()),
        ],
    );

    if app.data.packages.is_empty() {
        let empty_text = Paragraph::new("No packages installed.\n\nPress 'i' to import from Pi Agent\nor install via CLI:\n  pi-switch package install <id>")
            .style(Style::default().fg(theme.dim))
            .wrap(Wrap { trim: false });
        frame.render_widget(empty_text, chunks[1]);
        return;
    }

    let items: Vec<ListItem<'_>> = app
        .data
        .packages
        .iter()
        .map(|pkg| {
            let status_icon = if pkg.enabled { "✓" } else { " " };
            let status_color = if pkg.enabled { theme.ok } else { theme.dim };
            let installed_info = pkg
                .installed_at
                .as_ref()
                .map(|t| format!(" ({})", t))
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(&pkg.name, Style::default()),
                Span::styled(
                    format!(" v{}", pkg.version.as_deref().unwrap_or("unknown")),
                    Style::default().fg(theme.accent),
                ),
                Span::styled(installed_info, Style::default().fg(theme.dim)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(selection_style(theme))
        .highlight_symbol(super::highlight_symbol(theme))
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(format!("Packages ({})", app.data.packages.len())),
        );

    let mut state = ListState::default();
    state.select(Some(app.packages_idx));
    frame.render_stateful_widget(list, chunks[1], &mut state);
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
        &[
            ("↑↓", i18n::key_scroll()),
            ("←→", range_label(&app.data.stats_range)),
            ("r", i18n::key_refresh()),
            ("Esc", i18n::key_back()),
        ],
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
    overview_lines.push(label_line(
        app,
        i18n::stats_tokens(),
        token_summary(
            &stats.total_tokens,
            i18n::stats_input(),
            i18n::stats_output(),
            i18n::stats_cached(),
            i18n::stats_reasoning(),
        ),
    ));
    overview_lines.push(label_line(
        app,
        i18n::stats_cache_hit_rate(),
        stats.cache_hit_rate.clone(),
    ));
    overview_lines.push(label_line(
        app,
        i18n::stats_cost(),
        format_cost(stats.total_cost),
    ));

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
    frame.render_widget(
        Paragraph::new(detail_lines)
            .wrap(Wrap { trim: false })
            .scroll((app.stats_scroll, 0)),
        details_inner,
    );
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
            (
                "←→/Space",
                if app.settings_proxy_idx == 0 || app.settings_proxy_idx == 3 {
                    i18n::key_switch()
                } else {
                    ""
                },
            ),
            (
                "Enter",
                if app.settings_proxy_idx == 4 {
                    if i18n::is_zh() {
                        "编辑"
                    } else {
                        "Edit"
                    }
                } else if app.settings_proxy_idx == 1 || app.settings_proxy_idx == 2 {
                    i18n::key_edit()
                } else {
                    ""
                },
            ),
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

    // Disguise preset display
    let user_agent_presets = crate::tui::app::user_agent_presets();
    let user_agent_display = user_agent_presets
        .get(app.settings_user_agent_idx)
        .unwrap_or(&"?")
        .to_string();

    let rows_data: Vec<(&str, String)> = vec![
        (
            i18n::settings_lang_label(),
            if app.settings_lang_idx == 0 {
                i18n::settings_lang_en().to_string()
            } else {
                i18n::settings_lang_zh().to_string()
            },
        ),
        (i18n::settings_proxy_host(), proxy.host.clone()),
        (i18n::settings_proxy_port(), proxy.port.to_string()),
        (
            if i18n::is_zh() {
                "用户代理"
            } else {
                "User-Agent"
            },
            user_agent_display,
        ),
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

    let table = Table::new(rows, [Constraint::Length(label_width), Constraint::Min(10)])
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
                chunks[1]
                    .width
                    .saturating_sub(label_width)
                    .saturating_sub(2)
                    .max(1),
            );
            frame.set_cursor_position((x + cursor_x, y));
        }
    }
}

/// Format a cumulative token count for compact display (0, 999, 12.3K, 1.2M).
/// The same K/M/B style is planned for the web UI dashboard.
fn format_token_count(count: u64) -> String {
    const UNITS: [&str; 5] = ["", "K", "M", "B", "T"];
    let mut scaled = count as f64;
    let mut unit = 0usize;
    while scaled >= 1000.0 && unit < UNITS.len() - 1 {
        scaled /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        return format!("{count}");
    }
    let rounded = (scaled * 10.0).round() / 10.0;
    if rounded >= 1000.0 && unit < UNITS.len() - 1 {
        scaled = rounded / 1000.0;
        unit += 1;
    } else {
        scaled = rounded;
    }
    format!("{scaled:.1}{}", UNITS[unit])
}

/// Cost display sharing the web UI rules: "$0.00" for explicit zero, four
/// decimals (trailing zeros trimmed) under a dollar, two decimals up to
/// $999.99, K/M suffixes beyond. `None` (unknown) renders "-".
fn format_cost(cost: Option<f64>) -> String {
    let Some(cost) = cost else {
        return "-".to_string();
    };
    if cost == 0.0 {
        return "$0.00".to_string();
    }
    if cost < 1.0 {
        let fixed = format!("{cost:.4}");
        let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
        return format!("${trimmed}");
    }
    if cost < 1000.0 {
        return format!("${cost:.2}");
    }
    format!("${}", format_token_count(cost as u64))
}

/// Value shown for the cumulative-token line of the stats view. Renders
/// "-" when there is no token data at all, otherwise a readable
/// "12.3K input / 678 output · 4.2K cached / 345 reasoning" summary.
/// The dimension words are injected so the function stays
/// language-independent and testable.
fn token_summary(
    total: &TokenTotals,
    input_word: &str,
    output_word: &str,
    cached_word: &str,
    reasoning_word: &str,
) -> String {
    if total.total == 0 {
        return "-".to_string();
    }
    format!(
        "{} {input_word} / {} {output_word} · {} {cached_word} / {} {reasoning_word}",
        format_token_count(total.input),
        format_token_count(total.output),
        format_token_count(total.cached),
        format_token_count(total.reasoning),
    )
}

/// Localized label for the active stats time range.
fn range_label(range: &crate::tui::data::StatsRange) -> &'static str {
    use crate::tui::data::StatsRange;
    match range {
        StatsRange::All => i18n::stats_range_all(),
        StatsRange::Today => i18n::stats_range_today(),
        StatsRange::Last24h => i18n::stats_range_24h(),
        StatsRange::Last7d => i18n::stats_range_7d(),
    }
}

pub(super) fn render_failover_editor(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = content_block(
        app,
        if i18n::is_zh() {
            "故障转移链编辑"
        } else {
            "Failover Chain Editor"
        },
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let key_hints: &[(&str, &str)] = &[
        ("↑↓", i18n::key_move()),
        ("Space", if i18n::is_zh() { "勾选" } else { "Toggle" }),
        ("Ctrl+j/k", if i18n::is_zh() { "移动" } else { "Move" }),
        ("Enter/s", i18n::key_save()),
        ("Esc", i18n::key_back()),
    ];
    render_key_bar_center(frame, theme, chunks[0], key_hints);

    if app.failover_list.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                if i18n::is_zh() {
                    "无可用的供应商"
                } else {
                    "No available providers"
                },
                Style::default().fg(theme.dim),
            ))),
            chunks[1],
        );
        return;
    }

    // Render list with checkboxes
    let items: Vec<ListItem<'_>> = app
        .failover_list
        .iter()
        .map(|(name, selected)| {
            let checkbox = if *selected { "[✓]" } else { "[ ]" };
            let text = format!("  {} {}", checkbox, name);
            ListItem::new(text)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(selection_style(theme))
        .highlight_symbol(highlight_symbol(theme));

    let mut state = ListState::default();
    state.select(Some(app.failover_idx));
    frame.render_stateful_widget(list, chunks[1], &mut state);
}

#[cfg(test)]
mod tests {
    use super::{format_cost, format_token_count, token_summary};
    use crate::stats::TokenTotals;

    fn totals(input: u64, output: u64) -> TokenTotals {
        TokenTotals {
            input,
            output,
            total: input + output,
            cached: 0,
            reasoning: 0,
        }
    }

    fn totals_full(input: u64, output: u64, cached: u64, reasoning: u64) -> TokenTotals {
        TokenTotals {
            input,
            output,
            total: input + output,
            cached,
            reasoning,
        }
    }

    #[test]
    fn format_token_count_small_counts_are_plain() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
    }

    #[test]
    fn format_token_count_uses_readable_suffixes() {
        assert_eq!(format_token_count(1000), "1.0K");
        assert_eq!(format_token_count(12_345), "12.3K");
        assert_eq!(format_token_count(999_500), "999.5K");
        assert_eq!(format_token_count(12_345_678), "12.3M");
        assert_eq!(format_token_count(1_234_567_890), "1.2B");
        assert_eq!(format_token_count(1_234_567_890_123), "1.2T");
    }

    #[test]
    fn format_token_count_rounds_to_one_decimal() {
        assert_eq!(format_token_count(12_349), "12.3K");
        assert_eq!(format_token_count(12_351), "12.4K");
    }

    #[test]
    fn format_token_count_carries_over_at_thousand_boundary() {
        assert_eq!(format_token_count(999_950), "1.0M");
        assert_eq!(format_token_count(999_999), "1.0M");
    }

    #[test]
    fn token_summary_without_data_is_dash() {
        assert_eq!(
            token_summary(&totals(0, 0), "input", "output", "cached", "reasoning"),
            "-"
        );
    }

    #[test]
    fn token_summary_renders_input_and_output() {
        assert_eq!(
            token_summary(
                &totals(12_345, 678),
                "input",
                "output",
                "cached",
                "reasoning"
            ),
            "12.3K input / 678 output · 0 cached / 0 reasoning"
        );
    }

    #[test]
    fn token_summary_renders_cached_and_reasoning_dimensions() {
        assert_eq!(
            token_summary(
                &totals_full(12_345, 678, 4_200, 345),
                "input",
                "output",
                "cached",
                "reasoning",
            ),
            "12.3K input / 678 output · 4.2K cached / 345 reasoning"
        );
    }

    #[test]
    fn token_summary_zero_input_is_still_rendered() {
        assert_eq!(
            token_summary(
                &totals(0, 12_345_678),
                "input",
                "output",
                "cached",
                "reasoning",
            ),
            "0 input / 12.3M output · 0 cached / 0 reasoning"
        );
    }

    #[test]
    fn token_summary_accepts_localized_words() {
        assert_eq!(
            token_summary(
                &totals_full(1_234, 567, 300, 0),
                "输入",
                "输出",
                "缓存",
                "推理",
            ),
            "1.2K 输入 / 567 输出 · 300 缓存 / 0 推理"
        );
    }
    #[test]
    fn format_cost_renders_adaptive_precision() {
        assert_eq!(format_cost(None), "-");
        assert_eq!(format_cost(Some(0.0)), "$0.00");
        assert_eq!(format_cost(Some(0.0042)), "$0.0042");
        assert_eq!(format_cost(Some(12.34)), "$12.34");
        assert_eq!(format_cost(Some(1234.0)), "$1.2K");
    }

    #[test]
    fn format_cost_trims_sub_dollar_trailing_zeros() {
        assert_eq!(format_cost(Some(0.1)), "$0.1");
        assert_eq!(format_cost(Some(0.00420)), "$0.0042");
    }
}
