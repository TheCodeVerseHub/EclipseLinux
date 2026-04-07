/// Progress screen: shows a gauge/progress bar driven by the install thread.
use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};
use ratatui::Frame;

use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header};

use super::StepResult;

pub struct ProgressScreen {
    percent: u16,
    message: String,
}

impl ProgressScreen {
    pub fn new() -> Self {
        Self {
            percent: 0,
            message: "Preparing...".to_string(),
        }
    }

    pub fn update(&mut self, percent: u8, message: &str) {
        self.percent = percent as u16;
        self.message = message.to_string();
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " Installation in progress... Please wait. ");

        let box_area = centered_fixed(65, 10, content);
        let block = content_block("Installing Eclipse Linux");
        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(inner);

        let status = Paragraph::new(Line::from(Span::raw(&self.message)));
        frame.render_widget(status, rows[1]);

        let gauge = Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::DarkGray),
            )
            .percent(self.percent)
            .label(format!("{}%", self.percent));
        frame.render_widget(gauge, rows[3]);
    }

    pub fn handle_input(&self, _key: KeyEvent) -> StepResult {
        StepResult::Continue
    }
}
