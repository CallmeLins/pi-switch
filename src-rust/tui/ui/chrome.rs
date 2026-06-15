use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState,
};
use ratatui::Frame;

use crate::tui::app::{App, Focus};
use crate::tui::i18n;
use crate::tui::route::NavItem;
use crate::tui::theme::{palette_color, Theme};

use super::{display_width, highlight_symbol, pane_border_style, selection_style};

pub(super) fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme.dim));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let title_text = i18n::header_title();
    let title_width = display_width(title_text);
    let title_style = if theme.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let mut right_spans: Vec<Span<'_>> = Vec::new();

    if app.data.daemon.running {
        let port = app
            .data
            .daemon
            .port
            .unwrap_or(0);
        let proxy_style = if theme.no_color {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(theme.surface)
        };
        right_spans.push(Span::styled(i18n::header_proxy_running(port), proxy_style));
        right_spans.push(Span::raw(" "));
    }

    let current = app
        .data
        .config
        .current
        .as_deref()
        .unwrap_or("none")
        .to_string();
    let status_text = i18n::header_provider(&current);
    let available = inner.width.saturating_sub(title_width);
    let right_width: u16 = right_spans
        .iter()
        .map(|span| span.width() as u16)
        .sum::<u16>()
        + display_width(&status_text);
    if right_width <= available {
        right_spans.push(Span::styled(status_text, selection_style(theme)));
    }

    let title_area = Rect::new(inner.x, inner.y, title_width.min(inner.width), inner.height);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(title_text, title_style)))
            .alignment(Alignment::Left),
        title_area,
    );

    let right_area = Rect::new(
        title_area.right(),
        inner.y,
        inner.right().saturating_sub(title_area.right()),
        inner.height,
    );
    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        right_area,
    );
}

pub(super) fn nav_pane_width(theme: &Theme) -> u16 {
    const NAV_BORDER_WIDTH: u16 = 2;
    const NAV_ICON_COL_WIDTH: u16 = 4;
    const NAV_COL_SPACING: u16 = 1;
    const NAV_TEXT_MIN_WIDTH: u16 = 10;
    const NAV_TEXT_EXTRA_WIDTH: u16 = 2;
    let highlight_width = display_width(highlight_symbol(theme));

    let max_text_width = NavItem::ALL
        .iter()
        .map(|item| display_width(item.label()))
        .max()
        .unwrap_or(NAV_TEXT_MIN_WIDTH);

    let text_col_width = max_text_width
        .saturating_add(NAV_TEXT_EXTRA_WIDTH)
        .max(NAV_TEXT_MIN_WIDTH);

    NAV_BORDER_WIDTH
        .saturating_add(highlight_width)
        .saturating_add(NAV_ICON_COL_WIDTH)
        .saturating_add(NAV_COL_SPACING)
        .saturating_add(text_col_width)
}

pub(super) fn render_nav(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;
    let rows = NavItem::ALL.iter().map(|item| {
        Row::new(vec![
            Cell::from(format!(" {}", item.icon())),
            Cell::from(item.label()),
        ])
    });

    let table = Table::new(rows, [ratatui::layout::Constraint::Length(4), ratatui::layout::Constraint::Min(10)])
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(pane_border_style(theme, app.focus == Focus::Nav))
                .title(i18n::menu_title()),
        )
        .row_highlight_style(selection_style(theme))
        .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.nav_idx));
    frame.render_stateful_widget(table, area, &mut state);
}

pub(super) fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = &app.theme;

    let spans = if app.filter_active {
        vec![Span::styled(
            i18n::footer_filter(),
            Style::default().fg(theme.dim),
        )]
    } else if theme.no_color {
        vec![Span::styled(
            i18n::footer_no_color(),
            Style::default(),
        )]
    } else {
        let nav_bg = palette_color((101, 113, 160));
        let act_bg = palette_color((248, 248, 248));
        let nav_fg = palette_color((255, 255, 255));
        let act_fg = palette_color((108, 108, 108));
        let nav_key_style = Style::default()
            .fg(nav_fg)
            .bg(nav_bg)
            .add_modifier(Modifier::BOLD);
        let nav_desc_style = Style::default().fg(nav_fg).bg(nav_bg);
        let act_key_style = Style::default()
            .fg(act_fg)
            .bg(act_bg)
            .add_modifier(Modifier::BOLD);
        let act_desc_style = Style::default().fg(act_fg).bg(act_bg);
        let nav_sep = Span::styled("  ", nav_desc_style);
        let act_sep = Span::styled("  ", act_desc_style);

        let nav_items: &[(&str, &str)] = &[("←→", i18n::footer_nav_label()), ("↑↓", i18n::footer_nav_move())];
        let act_items: &[(&str, &str)] = &[
            ("/", i18n::footer_act_filter()),
            ("Esc", i18n::footer_act_back()),
            ("?", i18n::footer_act_help()),
            ("q", i18n::footer_act_quit()),
        ];

        let mut v = Vec::new();
        for (i, (key, desc)) in nav_items.iter().enumerate() {
            if i > 0 {
                v.push(nav_sep.clone());
            }
            v.push(Span::styled(format!(" {} ", key), nav_key_style));
            v.push(Span::styled(format!(" {}", desc), nav_desc_style));
        }
        v.push(Span::styled(" ", nav_desc_style));
        v.push(Span::raw(" "));
        for (i, (key, desc)) in act_items.iter().enumerate() {
            if i > 0 {
                v.push(act_sep.clone());
            }
            v.push(Span::styled(format!(" {} ", key), act_key_style));
            v.push(Span::styled(format!(" {}", desc), act_desc_style));
        }
        v.push(Span::styled(" ", act_desc_style));
        v
    };

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
