/// User account screen: username input with regex validation,
/// then masked password with confirmation. Empty password allowed with warning.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::{
    base_layout, centered_fixed, content_block, draw_footer, draw_header, error_style,
    warning_style,
};

use super::StepResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Username,
    UsernameError,
    Password,
    PasswordConfirm,
    EmptyPasswordWarning,
    PasswordMismatch,
}

pub struct UserAccountScreen {
    user_input: String,
    pass1: String,
    pass2: String,
    phase: Phase,
    error_msg: String,
}

impl UserAccountScreen {
    pub fn new() -> Self {
        Self {
            user_input: String::new(),
            pass1: String::new(),
            pass2: String::new(),
            phase: Phase::Username,
            error_msg: String::new(),
        }
    }

    pub fn username(&self) -> String {
        self.user_input.clone()
    }

    pub fn password(&self) -> Option<String> {
        if self.pass1.is_empty() {
            None
        } else {
            Some(self.pass1.clone())
        }
    }

    fn validate_username(&self) -> bool {
        let re = regex::Regex::new(r"^[a-z][a-z0-9_-]*$").unwrap();
        re.is_match(&self.user_input)
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);

        let box_area = centered_fixed(60, 10, content);
        let block = content_block("User Account");
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
            Phase::Username => {
                draw_footer(frame, footer, " [Enter] Confirm  [Esc] Back ");
                let prompt = Paragraph::new("Enter a username for the primary user account:");
                frame.render_widget(prompt, rows[1]);
                let display = format!("{}_", self.user_input);
                let input = Paragraph::new(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Cyan)),
                    Span::raw(display),
                ]));
                frame.render_widget(input, rows[3]);
            }
            Phase::UsernameError => {
                draw_footer(frame, footer, " [Enter] Try again ");
                let msg = Paragraph::new(Line::from(Span::styled(
                    &self.error_msg,
                    error_style(),
                )));
                frame.render_widget(msg, rows[2]);
            }
            Phase::Password => {
                draw_footer(frame, footer, " [Enter] Confirm  [Esc] Back to username ");
                let prompt =
                    Paragraph::new(format!("Enter password for {}:", self.user_input));
                frame.render_widget(prompt, rows[1]);
                let masked = "*".repeat(self.pass1.len()) + "_";
                let input = Paragraph::new(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Cyan)),
                    Span::raw(masked),
                ]));
                frame.render_widget(input, rows[3]);
            }
            Phase::PasswordConfirm => {
                draw_footer(frame, footer, " [Enter] Confirm  [Esc] Re-enter ");
                let prompt =
                    Paragraph::new(format!("Confirm password for {}:", self.user_input));
                frame.render_widget(prompt, rows[1]);
                let masked = "*".repeat(self.pass2.len()) + "_";
                let input = Paragraph::new(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Cyan)),
                    Span::raw(masked),
                ]));
                frame.render_widget(input, rows[3]);
            }
            Phase::EmptyPasswordWarning => {
                draw_footer(
                    frame,
                    footer,
                    " [y] Continue without password  [n] Go back ",
                );
                let warn = Paragraph::new(Line::from(Span::styled(
                    format!("No password set for {}. Continue?", self.user_input),
                    warning_style(),
                )));
                frame.render_widget(warn, rows[1]);
                let prompt = Paragraph::new("Continue without a password? (y/n)");
                frame.render_widget(prompt, rows[3]);
            }
            Phase::PasswordMismatch => {
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
            Phase::Username => match key.code {
                KeyCode::Enter => {
                    if self.user_input.is_empty() {
                        self.error_msg = "Username cannot be empty.".to_string();
                        self.phase = Phase::UsernameError;
                    } else if !self.validate_username() {
                        self.error_msg =
                            "Invalid username. Use lowercase letters, digits, hyphens, or underscores. Must start with a letter."
                                .to_string();
                        self.phase = Phase::UsernameError;
                    } else {
                        self.phase = Phase::Password;
                    }
                    StepResult::Continue
                }
                KeyCode::Esc => StepResult::Back,
                KeyCode::Backspace => {
                    self.user_input.pop();
                    StepResult::Continue
                }
                KeyCode::Char(c) => {
                    self.user_input.push(c);
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
            Phase::UsernameError => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.user_input.clear();
                    self.phase = Phase::Username;
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
            Phase::Password => match key.code {
                KeyCode::Enter => {
                    if self.pass1.is_empty() {
                        self.phase = Phase::EmptyPasswordWarning;
                    } else {
                        self.phase = Phase::PasswordConfirm;
                    }
                    StepResult::Continue
                }
                KeyCode::Esc => {
                    self.pass1.clear();
                    self.phase = Phase::Username;
                    StepResult::Continue
                }
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
            Phase::PasswordConfirm => match key.code {
                KeyCode::Enter => {
                    if self.pass1 == self.pass2 {
                        StepResult::Next
                    } else {
                        self.phase = Phase::PasswordMismatch;
                        StepResult::Continue
                    }
                }
                KeyCode::Esc => {
                    self.pass2.clear();
                    self.phase = Phase::Password;
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
            Phase::EmptyPasswordWarning => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => StepResult::Next,
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.phase = Phase::Password;
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
            Phase::PasswordMismatch => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.pass1.clear();
                    self.pass2.clear();
                    self.phase = Phase::Password;
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
        }
    }
}
