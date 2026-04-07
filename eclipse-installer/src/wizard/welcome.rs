/// Welcome screen: splash text with continue/quit prompt.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header};

use super::StepResult;

pub struct WelcomeScreen;

impl WelcomeScreen {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " [Enter] Continue  [q/Esc] Quit ");

        let box_area = centered_fixed(65, 15, content);
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Welcome to the Eclipse Linux Installer",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Eclipse Linux is a Void Linux-based distribution using the"),
            Line::from("dynamod init system."),
            Line::from(""),
            Line::from("This will guide you through installing Eclipse Linux to a disk."),
            Line::from(Span::styled(
                "All data on the selected disk will be destroyed.",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter to continue or Esc to quit.",
                Style::default().fg(Color::Gray),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .block(content_block("Eclipse Linux Installer"))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, box_area);
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Enter => StepResult::Next,
            KeyCode::Esc | KeyCode::Char('q') => StepResult::Quit,
            _ => StepResult::Continue,
        }
    }
}
