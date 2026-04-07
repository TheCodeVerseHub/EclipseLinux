/// Timezone selection: two-level menu (region then city) from /usr/share/zoneinfo/.
use std::fs;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use crate::config::TIMEZONE_REGIONS;
use crate::ui::{base_layout, centered_fixed, content_block, draw_footer, draw_header, highlight_style};

use super::StepResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Region,
    City,
}

pub struct TimezoneScreen {
    phase: Phase,
    regions: Vec<String>,
    cities: Vec<String>,
    region_state: ListState,
    city_state: ListState,
    selected_region: String,
}

impl TimezoneScreen {
    pub fn new() -> Self {
        let regions: Vec<String> = TIMEZONE_REGIONS
            .iter()
            .filter(|r| {
                std::path::Path::new(&format!("/usr/share/zoneinfo/{}", r)).is_dir()
            })
            .map(|r| r.to_string())
            .collect();

        let mut region_state = ListState::default();
        if !regions.is_empty() {
            region_state.select(Some(0));
        }

        Self {
            phase: Phase::Region,
            regions,
            cities: Vec::new(),
            region_state,
            city_state: ListState::default(),
            selected_region: String::new(),
        }
    }

    pub fn selected_timezone(&self) -> String {
        if self.selected_region.is_empty() {
            return "UTC".to_string();
        }
        if let Some(idx) = self.city_state.selected() {
            if let Some(city) = self.cities.get(idx) {
                return format!("{}/{}", self.selected_region, city);
            }
        }
        "UTC".to_string()
    }

    fn load_cities(&mut self) {
        self.cities.clear();
        let dir = format!("/usr/share/zoneinfo/{}", self.selected_region);
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut cities: Vec<String> = entries
                .flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            cities.sort();
            self.cities = cities;
        }
        if !self.cities.is_empty() {
            self.city_state.select(Some(0));
        } else {
            self.city_state.select(None);
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let (header, content, footer) = base_layout(area);
        draw_header(frame, header);

        match self.phase {
            Phase::Region => {
                draw_footer(
                    frame,
                    footer,
                    " [Up/Down] Select  [Enter] Confirm  [Esc] Back ",
                );
                let box_area = centered_fixed(50, 22, content);

                if self.regions.is_empty() {
                    let text = ratatui::widgets::Paragraph::new("No timezone regions found. Using UTC.")
                        .block(content_block("Timezone Region"));
                    frame.render_widget(text, box_area);
                    return;
                }

                let items: Vec<ListItem> = self
                    .regions
                    .iter()
                    .map(|r| ListItem::new(Line::from(format!("  {}", r))))
                    .collect();

                let list = List::new(items)
                    .block(content_block("Timezone Region"))
                    .highlight_style(highlight_style())
                    .highlight_symbol("▶ ");

                frame.render_stateful_widget(list, box_area, &mut self.region_state);
            }
            Phase::City => {
                draw_footer(
                    frame,
                    footer,
                    " [Up/Down] Select  [Enter] Confirm  [Esc] Back to region ",
                );
                let box_area = centered_fixed(50, 22, content);

                if self.cities.is_empty() {
                    let text =
                        ratatui::widgets::Paragraph::new("No cities found. Using UTC.")
                            .block(content_block("Timezone"));
                    frame.render_widget(text, box_area);
                    return;
                }

                let items: Vec<ListItem> = self
                    .cities
                    .iter()
                    .map(|c| ListItem::new(Line::from(format!("  {}", c))))
                    .collect();

                let list = List::new(items)
                    .block(content_block("Timezone"))
                    .highlight_style(highlight_style())
                    .highlight_symbol("▶ ");

                frame.render_stateful_widget(list, box_area, &mut self.city_state);
            }
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> StepResult {
        match self.phase {
            Phase::Region => match key.code {
                KeyCode::Up => {
                    if let Some(i) = self.region_state.selected() {
                        if i > 0 {
                            self.region_state.select(Some(i - 1));
                        }
                    }
                    StepResult::Continue
                }
                KeyCode::Down => {
                    if let Some(i) = self.region_state.selected() {
                        if i + 1 < self.regions.len() {
                            self.region_state.select(Some(i + 1));
                        }
                    }
                    StepResult::Continue
                }
                KeyCode::Enter => {
                    if self.regions.is_empty() {
                        return StepResult::Next;
                    }
                    if let Some(idx) = self.region_state.selected() {
                        self.selected_region = self.regions[idx].clone();
                        self.load_cities();
                        self.phase = Phase::City;
                    }
                    StepResult::Continue
                }
                KeyCode::Esc => StepResult::Back,
                _ => StepResult::Continue,
            },
            Phase::City => match key.code {
                KeyCode::Up => {
                    if let Some(i) = self.city_state.selected() {
                        if i > 0 {
                            self.city_state.select(Some(i - 1));
                        }
                    }
                    StepResult::Continue
                }
                KeyCode::Down => {
                    if let Some(i) = self.city_state.selected() {
                        if i + 1 < self.cities.len() {
                            self.city_state.select(Some(i + 1));
                        }
                    }
                    StepResult::Continue
                }
                KeyCode::Enter => {
                    if self.cities.is_empty() {
                        self.phase = Phase::Region;
                        return StepResult::Continue;
                    }
                    StepResult::Next
                }
                KeyCode::Esc => {
                    self.phase = Phase::Region;
                    StepResult::Continue
                }
                _ => StepResult::Continue,
            },
        }
    }
}
