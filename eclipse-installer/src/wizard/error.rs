/// Error screen: displays the error message and log path.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::LOG_PATH;
use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header, error_style};

use super::StepResult;

pub struct ErrorScreen {
    message: String,
}

impl ErrorScreen {
    pub fn new() -> Self {
        Self {
            message: String::new(),
        }
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message = msg.to_string();
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " [q/Esc] Quit ");

        let box_area = centered_fixed(70, 16, content);

        let mut text = vec![
            Line::from(""),
            Line::from(Span::styled("Installation Failed", error_style())),
            Line::from(""),
        ];

        for line in self.message.lines() {
            text.push(Line::from(line.to_string()));
        }

        text.push(Line::from(""));
        text.push(Line::from(format!("Check {} for details.", LOG_PATH)));
        text.push(Line::from(""));
        text.push(Line::from("Press q or Esc to quit."));

        let paragraph = Paragraph::new(text)
            .block(content_block("Error"))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, box_area);
    }

    pub fn handle_input(&self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => StepResult::Quit,
            _ => StepResult::Continue,
        }
    }
}
