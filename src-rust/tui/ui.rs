mod chrome;
mod overlay;
mod pages;
mod profiles;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::app::{App, Focus};
use super::route::Route;
use super::theme::Theme;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(chrome::nav_pane_width(&app.theme)),
            Constraint::Min(0),
        ])
        .split(chunks[1]);

    chrome::render_header(frame, app, chunks[0]);
    chrome::render_nav(frame, app, body[0]);
    render_content(frame, app, body[1]);
    chrome::render_footer(frame, app, chunks[2]);

    overlay::render_overlay(frame, app, body[1]);
    overlay::render_toast(frame, app, body[1]);
}

fn render_content(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    match app.route.clone() {
        Route::Home => pages::render_home(frame, app, area),
        Route::Profiles => profiles::render_profiles(frame, app, area),
        Route::ProfileDetail(name) => profiles::render_profile_detail(frame, app, area, &name),
        Route::Form => profiles::render_form(frame, app, area),
        Route::Presets => pages::render_presets(frame, app, area),
        Route::Proxy => pages::render_proxy(frame, app, area),
        Route::Stats => pages::render_stats(frame, app, area),
        Route::Backups => pages::render_backups(frame, app, area),
        Route::Settings => pages::render_settings(frame, app, area),
    }
}

// ─── Shared styles ────────────────────────────────────────

pub(super) fn selection_style(theme: &Theme) -> Style {
    if theme.no_color {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    }
}

pub(super) fn highlight_symbol(theme: &Theme) -> &'static str {
    if theme.no_color {
        "> "
    } else {
        ""
    }
}

pub(super) fn pane_border_style(theme: &Theme, focused: bool) -> Style {
    if focused {
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dim)
    }
}

pub(super) fn content_block(app: &App, title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(
            &app.theme,
            app.focus == Focus::Content,
        ))
        .title(title.into())
}

pub(super) fn display_width(text: &str) -> u16 {
    Span::raw(text).width() as u16
}

/// Centered single-line key hint bar rendered at the top of a content pane.
pub(super) fn key_bar_line<'a>(theme: &Theme, items: &[(&'a str, &'a str)]) -> Line<'a> {
    let key_style = if theme.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
    };
    let desc_style = Style::default().fg(theme.dim);

    let mut spans = Vec::new();
    for (i, (key, desc)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", desc_style));
        }
        spans.push(Span::styled(format!("{key} "), key_style));
        spans.push(Span::styled(*desc, desc_style));
    }
    Line::from(spans)
}

pub(super) fn render_key_bar_center(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    items: &[(&str, &str)],
) {
    frame.render_widget(
        Paragraph::new(key_bar_line(theme, items))
            .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

pub(super) fn centered_rect_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
