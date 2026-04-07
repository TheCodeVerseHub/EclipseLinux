/// Root password screen: masked input with confirmation, allows empty with warning.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::{
    base_layout, centered_fixed, content_block, draw_footer, draw_header, warning_style,
};

use super::StepResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Enter,
    Confirm,
    EmptyWarning,
    Mismatch,
}

pub struct RootPasswordScreen {
    pass1: String,
    pass2: String,
    phase: Phase,
}

impl RootPasswordScreen {
    pub fn new() -> Self {
        Self {
            pass1: String::new(),
            pass2: String::new(),
            phase: Phase::Enter,
        }
    }

    pub fn password(&self) -> Option<String> {
        if self.pass1.is_empty() {
            None
        } else {
            Some(self.pass1.clone())
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);

        let box_area = centered_fixed(55, 10, content);
        let block = content_block("Root Password");
        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        match self.phase {
            Phase::Enter => {
                draw_footer(frame, footer, " [Enter] Confirm  [Esc] Back ");
                let prompt = Paragraph::new("Enter root password (empty for none):");
                frame.render_widget(prompt, rows[1]);
                let masked = "*".repeat(self.pass1.len()) + "_";
                let input = Paragraph::new(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Cyan)),
                    Span::raw(masked),
                ]));
                frame.render_widget(input, rows[3]);
            }
            Phase::Confirm => {
                draw_footer(frame, footer, " [Enter] Confirm  [Esc] Re-enter ");
                let prompt = Paragraph::new("Confirm root password:");
                frame.render_widget(prompt, rows[1]);
                let masked = "*".repeat(self.pass2.len()) + "_";
                let input = Paragraph::new(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Cyan)),
                    Span::raw(masked),
                ]));
                frame.render_widget(input, rows[3]);
            }
            Phase::EmptyWarning => {
                draw_footer(
                    frame,
                    footer,
                    " [y] Continue without password  [n] Go back ",
                );
                let warn = Paragraph::new(Line::from(Span::styled(
                    "No password set. Root will have no password.",
                    warning_style(),
                )));
                frame.render_widget(warn, rows[1]);
                let prompt = Paragraph::new("Continue without a password? (y/n)");
                frame.render_widget(prompt, rows[3]);
            }
            Phase::Mismatch => {
                draw_footer(frame, footer, " [Enter] Try again ");
                let msg = Paragraph::new(Line::from(Span::styled(
                    "Passwords do not match. Press Enter to try again.",
                    warning_style(),
                )));
                frame.render_widget(msg, rows[2]);
            }
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> StepResult {
        match self.phase {
            Phase::Enter => match key.code {
                KeyCode::Enter => {
                    if self.pass1.is_empty() {
                        self.phase = Phase::EmptyWarning;
                    } else {
                        self.phase = Phase::Confirm;
                    }
                    StepResult::Continue
                }
                KeyCode::Esc => StepResult::Back,
                KeyCode::Backspace => {
                    self.pass1.pop();
                    StepResult::Continue
                }
                KeyCode::Char(c) => {
                    self.pass1.push(c);
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
            Phase::Confirm => match key.code {
                KeyCode::Enter => {
                    if self.pass1 == self.pass2 {
                        StepResult::Next
                    } else {
                        self.phase = Phase::Mismatch;
                        StepResult::Continue
                    }
                }
                KeyCode::Esc => {
                    self.pass2.clear();
                    self.phase = Phase::Enter;
                    StepResult::Continue
                }
                KeyCode::Backspace => {
                    self.pass2.pop();
                    StepResult::Continue
                }
                KeyCode::Char(c) => {
                    self.pass2.push(c);
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
            Phase::EmptyWarning => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => StepResult::Next,
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.phase = Phase::Enter;
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
            Phase::Mismatch => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.pass1.clear();
                    self.pass2.clear();
                    self.phase = Phase::Enter;
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
        }
    }
}
