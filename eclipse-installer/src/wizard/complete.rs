/// Completion screen: success message with reboot prompt.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header};

use super::StepResult;

pub struct CompleteScreen {
    reboot: bool,
}

impl CompleteScreen {
    pub fn new() -> Self {
        Self { reboot: false }
    }

    pub fn should_reboot(&self) -> bool {
        self.reboot
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " [y] Reboot  [n/q] Quit ");

        let box_area = centered_fixed(60, 14, content);
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Eclipse Linux has been installed successfully!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("You can now reboot into your new system."),
            Line::from("Remove the installation media before rebooting."),
            Line::from(""),
            Line::from(""),
            Line::from("Reboot now? (y/n)"),
        ];

        let paragraph = Paragraph::new(text)
            .block(content_block("Installation Complete"))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, box_area);
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.reboot = true;
                StepResult::Quit
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') | KeyCode::Esc => {
                self.reboot = false;
                StepResult::Quit
            }
            _ => StepResult::Continue,
        }
    }
}
