use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::config::BACKTITLE;

pub fn base_style() -> Style {
    Style::default().fg(Color::White).bg(Color::Black)
}

pub fn highlight_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn title_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn warning_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub fn error_style() -> Style {
    Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD)
}

/// Splits the frame into header (1 line), content (flexible), footer (1 line).
pub fn base_layout(area: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
    (chunks[0], chunks[1], chunks[2])
}

pub fn draw_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![Span::styled(
        BACKTITLE,
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    )]))
    .style(Style::default().bg(Color::Blue))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(header, area);
}

pub fn draw_footer(frame: &mut Frame, area: Rect, hints: &str) {
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        hints,
        Style::default().fg(Color::Black).bg(Color::Cyan),
    )]))
    .style(Style::default().bg(Color::Cyan))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(footer, area);
}

/// Creates a centered rectangle of the given percentage width and height.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vert = Layout::default()
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
        .split(vert[1])[1]
}

/// Creates a centered rectangle with fixed dimensions, clamped to the available area.
pub fn centered_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Standard content block with a border and title.
pub fn content_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {} ", title),
            title_style(),
        ))
}
