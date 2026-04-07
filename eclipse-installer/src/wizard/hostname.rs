/// Hostname input screen with default "eclipse" and non-empty validation.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::DEFAULT_HOSTNAME;
use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header};

use super::StepResult;

pub struct HostnameScreen {
    input: String,
    cursor_pos: usize,
}

impl HostnameScreen {
    pub fn new() -> Self {
        Self {
            input: DEFAULT_HOSTNAME.to_string(),
            cursor_pos: DEFAULT_HOSTNAME.len(),
        }
    }

    pub fn value(&self) -> String {
        let v = self.input.trim().to_string();
        if v.is_empty() {
            DEFAULT_HOSTNAME.to_string()
        } else {
            v
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " [Enter] Confirm  [Esc] Back ");

        let box_area = centered_fixed(55, 9, content);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(content_block("Hostname").inner(box_area));

        let block = content_block("Hostname");
        frame.render_widget(block, box_area);

        let prompt = Paragraph::new(Line::from("Enter a hostname for this system:"));
        frame.render_widget(prompt, inner[1]);

        let display = format!("{}_", self.input);
        let input_line = Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(&display),
        ]));
        frame.render_widget(input_line, inner[3]);
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Enter => StepResult::Next,
            KeyCode::Esc => StepResult::Back,
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.input.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
                StepResult::Continue
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                StepResult::Continue
            }
            _ => StepResult::Continue,
        }
    }
}
