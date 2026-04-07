/// Filesystem selection screen: radio list of ext4, Btrfs, XFS.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use crate::config::FsType;
use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header, highlight_style};

use super::StepResult;

pub struct FilesystemScreen {
    state: ListState,
}

impl FilesystemScreen {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self { state }
    }

    pub fn selected(&self) -> FsType {
        match self.state.selected().unwrap_or(0) {
            0 => FsType::Ext4,
            1 => FsType::Btrfs,
            2 => FsType::Xfs,
            _ => FsType::Ext4,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(
            frame,
            footer,
            " [Up/Down] Select  [Enter] Confirm  [Esc] Back ",
        );

        let box_area = centered_fixed(55, 10, content);

        let items: Vec<ListItem> = FsType::ALL
            .iter()
            .map(|fs| ListItem::new(Line::from(format!("  {}", fs.description()))))
            .collect();

        let list = List::new(items)
            .block(content_block("Filesystem"))
            .highlight_style(highlight_style())
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, box_area, &mut self.state);
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> StepResult {
        match key.code {
            KeyCode::Up => {
                if let Some(i) = self.state.selected() {
                    if i > 0 {
                        self.state.select(Some(i - 1));
                    }
                }
                StepResult::Continue
            }
            KeyCode::Down => {
                if let Some(i) = self.state.selected() {
                    if i + 1 < FsType::ALL.len() {
                        self.state.select(Some(i + 1));
                    }
                }
                StepResult::Continue
            }
            KeyCode::Enter => StepResult::Next,
            KeyCode::Esc => StepResult::Back,
            _ => StepResult::Continue,
        }
    }
}
