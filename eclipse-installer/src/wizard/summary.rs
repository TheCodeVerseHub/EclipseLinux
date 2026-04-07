/// Installation summary screen: displays all collected values,
/// requires final yes/no confirmation before proceeding.
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

pub struct SummaryScreen;

impl SummaryScreen {
    pub fn draw(&self, frame: &mut Frame, area: Rect, config: &InstallConfig) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(
            frame,
            footer,
            " [y/Enter] Proceed  [n/Esc] Go back ",
        );

        let box_area = centered_fixed(65, 20, content);

        let root_pass_str = if config.root_password.is_some() {
            "(set)"
        } else {
            "(none)"
        };
        let user_pass_str = if config.user_password.is_some() {
            "(set)"
        } else {
            "(none)"
        };

        let text = vec![
            Line::from(""),
            Line::from("The following will be performed:"),
            Line::from(""),
            Line::from(format!("  Disk:        {}", config.disk)),
            Line::from(format!("  Boot mode:   {}", config.boot_mode_str())),
            Line::from(format!("  Filesystem:  {}", config.fs_type)),
            Line::from(format!("  Hostname:    {}", config.hostname)),
            Line::from(format!("  User:        {}", config.username)),
            Line::from(format!("  Timezone:    {}", config.timezone)),
            Line::from(format!("  Root pass:   {}", root_pass_str)),
            Line::from(format!("  User pass:   {}", user_pass_str)),
            Line::from(""),
            Line::from(Span::styled(
                format!("ALL DATA ON {} WILL BE DESTROYED.", config.disk),
                warning_style(),
            )),
            Line::from(""),
            Line::from("Proceed with installation?"),
        ];

        let paragraph = Paragraph::new(text)
            .block(content_block("Installation Summary"))
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
