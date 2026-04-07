/// Disk confirmation screen: shows selected disk and requires explicit yes.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::InstallConfig;
use crate::ui::{
    base_layout, centered_fixed, content_block, draw_footer, draw_header, warning_style,
};

use super::StepResult;

pub struct ConfirmScreen;

impl ConfirmScreen {
    pub fn draw(&self, frame: &mut Frame, area: Rect, config: &InstallConfig) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " [y/Enter] Confirm  [n/Esc] Go back ");

        let box_area = centered_fixed(65, 12, content);
        let text = vec![
            Line::from(""),
            Line::from("You selected:"),
            Line::from(""),
            Line::from(Span::raw(format!("  {}", config.disk_info))),
            Line::from(""),
            Line::from(Span::styled(
                "ALL DATA ON THIS DISK WILL BE DESTROYED.",
                warning_style(),
            )),
            Line::from(""),
            Line::from("Are you sure?"),
        ];

        let paragraph = Paragraph::new(text)
            .block(content_block("Confirm Disk"))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, box_area);
    }

    pub fn handle_input(&self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => StepResult::Next,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => StepResult::Back,
            _ => StepResult::Continue,
        }
    }
}
