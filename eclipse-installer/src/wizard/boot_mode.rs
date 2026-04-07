/// Boot mode screen: displays detected UEFI/BIOS mode and partition layout.
/// This is a read-only informational screen.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::InstallConfig;
use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header};

use super::StepResult;

pub struct BootModeScreen;

impl BootModeScreen {
    pub fn draw(&self, frame: &mut Frame, area: Rect, config: &InstallConfig) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(frame, footer, " [Enter] Continue  [Esc] Back ");

        let box_area = centered_fixed(65, 14, content);

        let layout_lines = if config.uefi {
            vec![
                Line::from("  GPT partition table"),
                Line::from("  512MB EFI System Partition (FAT32)"),
                Line::from("  Remaining space: root partition"),
            ]
        } else {
            vec![
                Line::from("  GPT partition table"),
                Line::from("  2MB BIOS boot partition"),
                Line::from("  Remaining space: root partition"),
            ]
        };

        let mut text = vec![
            Line::from(""),
            Line::from(format!(
                "Detected boot mode: {}",
                config.boot_mode_str()
            )),
            Line::from(""),
            Line::from("The disk will be partitioned accordingly:"),
            Line::from(""),
        ];
        text.extend(layout_lines);
        text.push(Line::from(""));
        text.push(Line::from(Span::raw("Press Enter to continue.")));

        let paragraph = Paragraph::new(text)
            .block(content_block("Boot Mode"))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, box_area);
    }

    pub fn handle_input(&self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Enter => StepResult::Next,
            KeyCode::Esc => StepResult::Back,
            _ => StepResult::Continue,
        }
    }
}
