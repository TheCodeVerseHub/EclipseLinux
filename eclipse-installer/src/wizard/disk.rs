/// Disk selection screen: lists block devices from /sys/block/.
use std::fs;
use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::config::DiskInfo;
use crate::ui::{
    base_layout, centered_fixed, content_block, draw_footer, draw_header, highlight_style,
};

use super::StepResult;

pub struct DiskScreen {
    disks: Vec<DiskInfo>,
    state: ListState,
    scanned: bool,
}

impl DiskScreen {
    pub fn new() -> Self {
        Self {
            disks: Vec::new(),
            state: ListState::default(),
            scanned: false,
        }
    }

    pub fn selected_disk(&self) -> Option<&DiskInfo> {
        self.state.selected().and_then(|i| self.disks.get(i))
    }

    fn scan_disks(&mut self) {
        self.disks.clear();
        let mounted = mounted_devices();

        for pattern in &["sd", "vd", "nvme"] {
            let entries = match fs::read_dir("/sys/block") {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with(pattern) {
                    continue;
                }
                let dev_path = format!("/dev/{}", name);
                if !std::path::Path::new(&dev_path).exists() {
                    continue;
                }
                if mounted.iter().any(|m| m.starts_with(&dev_path)) {
                    continue;
                }

                let sys_path = format!("/sys/block/{}", name);
                let size_bytes: u64 = fs::read_to_string(format!("{}/size", sys_path))
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(0);
                let size_gb = size_bytes * 512 / 1_073_741_824;
                if size_gb < 1 {
                    continue;
                }

                let model = fs::read_to_string(format!("{}/device/model", sys_path))
                    .unwrap_or_else(|_| "Unknown".to_string())
                    .trim()
                    .replace("  ", " ");
                let model = if model.is_empty() {
                    "Unknown".to_string()
                } else {
                    model
                };

                self.disks.push(DiskInfo {
                    path: dev_path,
                    size_gb,
                    model,
                });
            }
        }

        if !self.disks.is_empty() {
            self.state.select(Some(0));
        }
        self.scanned = true;
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        if !self.scanned {
            self.scan_disks();
        }

        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);
        draw_footer(
            frame,
            footer,
            " [Up/Down] Select  [Enter] Confirm  [Esc] Back ",
        );

        let box_area = centered_fixed(70, 20, content);

        if self.disks.is_empty() {
            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No suitable disks found.",
                    crate::ui::error_style(),
                )),
                Line::from(""),
                Line::from("Press Esc to go back."),
            ])
            .block(content_block("Select Disk"))
            .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(text, box_area);
            return;
        }

        let items: Vec<ListItem> = self
            .disks
            .iter()
            .map(|d| {
                ListItem::new(Line::from(format!(
                    "  {} - {}GB - {}",
                    d.path, d.size_gb, d.model
                )))
            })
            .collect();

        let list = List::new(items)
            .block(content_block("Select Disk"))
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
                    if i + 1 < self.disks.len() {
                        self.state.select(Some(i + 1));
                    }
                }
                StepResult::Continue
            }
            KeyCode::Enter => {
                if self.state.selected().is_some() && !self.disks.is_empty() {
                    StepResult::Next
                } else {
                    StepResult::Continue
                }
            }
            KeyCode::Esc => StepResult::Back,
            _ => StepResult::Continue,
        }
    }
}

/// Returns a list of device paths that are currently mounted.
fn mounted_devices() -> Vec<String> {
    let output = Command::new("mount").output();
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .filter_map(|line| {
                    let dev = line.split_whitespace().next()?;
                    if dev.starts_with("/dev/") {
                        Some(dev.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

impl DiskScreen {
    /// Force a rescan next time draw is called.
    pub fn invalidate(&mut self) {
        self.scanned = false;
    }
}
