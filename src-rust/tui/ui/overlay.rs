use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::{App, LoadingKind, Overlay, ToastKind};
use crate::tui::i18n;

use super::{centered_rect, centered_rect_fixed, display_width, render_key_bar_center};

const OVERLAY_FIXED_MD: (u16, u16) = (60, 9);
const TOAST_MIN_WIDTH: u16 = 28;
const TOAST_MAX_WIDTH: u16 = 72;
const TOAST_MIN_HEIGHT: u16 = 5;

pub(super) fn render_overlay(frame: &mut Frame<'_>, app: &App, content_area: Rect) {
    match &app.overlay {
        Overlay::None => {}
        Overlay::Help => render_help(frame, app),
        Overlay::Confirm(confirm) => {
            render_confirm(frame, app, content_area, &confirm.title, &confirm.message)
        }
        Overlay::Loading(kind) => render_loading(frame, app, kind),
    }
}

fn render_help(frame: &mut Frame<'_>, app: &App) {
    let theme = &app.theme;
    let area = centered_rect(90, 90, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme.dim))
        .title(i18n::help_title());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    render_key_bar_center(frame, theme, chunks[0], &[("Esc", i18n::key_close())]);

    let section = |title: &str| -> Line<'static> {
        Line::from(Span::styled(
            format!("  {title}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let item = |key: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("    {key:<14}"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc.to_string(), Style::default().fg(theme.dim)),
        ])
    };

    let lines = vec![
        Line::default(),
        section(i18n::help_section_global()),
        item("←→ / h l", i18n::help_arrow_keys()),
        item("↑↓ / j k", i18n::help_updown_keys()),
        item("Enter", i18n::help_enter()),
        item("Esc", i18n::help_esc()),
        item("q", i18n::help_q()),
        item("Ctrl+C", i18n::help_ctrl_c()),
        item("?", i18n::help_question()),
        Line::default(),
        section(i18n::help_section_profiles()),
        item("Enter", i18n::help_profiles_enter()),
        item("Space / s", i18n::help_profiles_space()),
        item("a", i18n::help_profiles_a()),
        item("e", i18n::help_profiles_e()),
        item("c", i18n::help_profiles_c()),
        item("d", i18n::help_profiles_d()),
        item("/", i18n::help_profiles_slash()),
        item("r", i18n::help_profiles_r()),
        Line::default(),
        section(i18n::help_section_form()),
        item("Tab", i18n::help_form_tab()),
        item("Enter", i18n::help_form_enter()),
        item("Space", i18n::help_form_space()),
        item("Ctrl+S", i18n::help_form_ctrl_s()),
        item("Esc", i18n::help_form_esc()),
        Line::default(),
        section(i18n::help_section_editing()),
        item("Ctrl+A / E", i18n::help_edit_ctrl_ae()),
        item("Ctrl+B / F", i18n::help_edit_ctrl_bf()),
        item("Alt+B / F", i18n::help_edit_alt_bf()),
        item("Ctrl+W", i18n::help_edit_ctrl_w()),
        item("Ctrl+U / K", i18n::help_edit_ctrl_uk()),
        Line::default(),
        section(i18n::help_section_presets()),
        item("Enter", i18n::help_presets_enter()),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn render_confirm(
    frame: &mut Frame<'_>,
    app: &App,
    content_area: Rect,
    title: &str,
    message: &str,
) {
    let theme = &app.theme;
    let area = centered_rect_fixed(OVERLAY_FIXED_MD.0, OVERLAY_FIXED_MD.1, content_area);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .title(format!(" {title} "));
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
        &[("Enter", i18n::key_confirm()), ("Esc", i18n::key_cancel())],
    );

    let body = vertically_centered_text(chunks[1], message, inner.width);
    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        body,
    );
}

fn render_loading(frame: &mut Frame<'_>, app: &App, kind: &LoadingKind) {
    let theme = &app.theme;
    let message = kind.message();
    let width = (display_width(message) as u16 + 8).clamp(TOAST_MIN_WIDTH, TOAST_MAX_WIDTH);
    let area = centered_rect_fixed(width, 5, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let tick_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() / 80) % spinner.len() as u128;
    let spinner_char = spinner[tick_idx as usize];

    frame.render_widget(
        Paragraph::new(format!("{} {}", spinner_char, message))
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.accent)),
        inner,
    );
}

fn vertically_centered_text(area: Rect, message: &str, width: u16) -> Rect {
    let lines = wrap_message_lines(message, width.max(1)).len() as u16;
    let height = lines.min(area.height).max(1);
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(area.x, y, area.width, height)
}

fn wrap_message_lines(message: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    let mut wrapped = Vec::new();
    for raw_line in message.lines() {
        let mut current = String::new();
        let mut current_width = 0usize;
        for word in raw_line.split_whitespace() {
            let word_width = display_width(word) as usize;
            let sep = usize::from(!current.is_empty());
            if current_width + sep + word_width > width && !current.is_empty() {
                wrapped.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if !current.is_empty() {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word_width;
        }
        wrapped.push(current);
    }
    if wrapped.is_empty() {
        wrapped.push(String::new());
    }
    wrapped
}

pub(super) fn render_toast(frame: &mut Frame<'_>, app: &App, content_area: Rect) {
    let Some(toast) = &app.toast else {
        return;
    };
    let theme = &app.theme;

    let (prefix, color) = match toast.kind {
        ToastKind::Info => ("ℹ", theme.accent),
        ToastKind::Success => ("✓", theme.accent),
        ToastKind::Warning => ("⚠", theme.warn),
        ToastKind::Error => ("✗", theme.err),
    };
    let message = format!("{prefix} {}", toast.message);
    let area = toast_rect(content_area, &message);

    frame.render_widget(Clear, area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.surface));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let text_style = if theme.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(color)
            .bg(theme.surface)
            .add_modifier(Modifier::BOLD)
    };

    let body = vertically_centered_text(inner, &message, inner.width);
    frame.render_widget(
        Paragraph::new(message.clone())
            .alignment(Alignment::Center)
            .style(text_style)
            .wrap(Wrap { trim: false }),
        body,
    );
}

fn toast_rect(content_area: Rect, message: &str) -> Rect {
    let max_width = content_area
        .width
        .saturating_sub(4)
        .max(1)
        .min(TOAST_MAX_WIDTH);
    let min_width = TOAST_MIN_WIDTH.min(max_width);
    let content_width = message
        .lines()
        .map(|line| display_width(line))
        .max()
        .unwrap_or(0);
    let width = content_width.saturating_add(8).clamp(min_width, max_width);

    let inner_width = width.saturating_sub(2).max(1);
    let wrapped_lines = wrap_message_lines(message, inner_width).len() as u16;
    let max_height = content_area.height.saturating_sub(4).max(1);
    let min_height = TOAST_MIN_HEIGHT.min(max_height);
    let height = wrapped_lines
        .saturating_add(2)
        .max(min_height)
        .min(max_height);

    centered_rect_fixed(width, height, content_area)
}
