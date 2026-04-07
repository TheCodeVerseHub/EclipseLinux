pub mod boot_mode;
pub mod complete;
pub mod confirm;
pub mod disk;
pub mod error;
pub mod filesystem;
pub mod hostname;
pub mod progress;
pub mod root_password;
pub mod summary;
pub mod timezone;
pub mod user_account;
pub mod welcome;

use std::sync::mpsc;

use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::config::InstallConfig;
use crate::install::{self, Progress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    Continue,
    Next,
    Back,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Disk,
    DiskConfirm,
    BootMode,
    Filesystem,
    Hostname,
    RootPassword,
    UserAccount,
    Timezone,
    Summary,
    Installing,
    Complete,
    Error,
}

const STEP_ORDER: &[Step] = &[
    Step::Welcome,
    Step::Disk,
    Step::DiskConfirm,
    Step::BootMode,
    Step::Filesystem,
    Step::Hostname,
    Step::RootPassword,
    Step::UserAccount,
    Step::Timezone,
    Step::Summary,
    Step::Installing,
    Step::Complete,
];

/// Top-level wizard that owns all per-step state and the install config.
pub struct Wizard {
    step_idx: usize,
    pub config: InstallConfig,

    pub welcome: welcome::WelcomeScreen,
    pub disk: disk::DiskScreen,
    pub confirm: confirm::ConfirmScreen,
    pub boot_mode: boot_mode::BootModeScreen,
    pub filesystem: filesystem::FilesystemScreen,
    pub hostname: hostname::HostnameScreen,
    pub root_password: root_password::RootPasswordScreen,
    pub user_account: user_account::UserAccountScreen,
    pub timezone: timezone::TimezoneScreen,
    pub summary: summary::SummaryScreen,
    pub progress: progress::ProgressScreen,
    pub complete: complete::CompleteScreen,
    pub error: error::ErrorScreen,

    install_rx: Option<mpsc::Receiver<Progress>>,
}

impl Wizard {
    pub fn new(config: InstallConfig) -> Self {
        Self {
            step_idx: 0,
            config,
            welcome: welcome::WelcomeScreen,
            disk: disk::DiskScreen::new(),
            confirm: confirm::ConfirmScreen,
            boot_mode: boot_mode::BootModeScreen,
            filesystem: filesystem::FilesystemScreen::new(),
            hostname: hostname::HostnameScreen::new(),
            root_password: root_password::RootPasswordScreen::new(),
            user_account: user_account::UserAccountScreen::new(),
            timezone: timezone::TimezoneScreen::new(),
            summary: summary::SummaryScreen,
            progress: progress::ProgressScreen::new(),
            complete: complete::CompleteScreen::new(),
            error: error::ErrorScreen::new(),
            install_rx: None,
        }
    }

    fn current_step(&self) -> Step {
        STEP_ORDER[self.step_idx]
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match self.current_step() {
            Step::Welcome => self.welcome.draw(frame, area),
            Step::Disk => self.disk.draw(frame, area),
            Step::DiskConfirm => self.confirm.draw(frame, area, &self.config),
            Step::BootMode => self.boot_mode.draw(frame, area, &self.config),
            Step::Filesystem => self.filesystem.draw(frame, area),
            Step::Hostname => self.hostname.draw(frame, area),
            Step::RootPassword => self.root_password.draw(frame, area),
            Step::UserAccount => self.user_account.draw(frame, area),
            Step::Timezone => self.timezone.draw(frame, area),
            Step::Summary => self.summary.draw(frame, area, &self.config),
            Step::Installing => self.progress.draw(frame, area),
            Step::Complete => self.complete.draw(frame, area),
            Step::Error => self.error.draw(frame, area),
        }
    }

    /// Handle a key event. Returns true if the application should quit.
    pub fn handle_input(&mut self, key: KeyEvent) -> bool {
        let result = match self.current_step() {
            Step::Welcome => self.welcome.handle_input(key),
            Step::Disk => {
                let r = self.disk.handle_input(key);
                if r == StepResult::Next {
                    if let Some(d) = self.disk.selected_disk() {
                        self.config.disk = d.path.clone();
                        self.config.disk_info = d.to_string();
                    }
                }
                r
            }
            Step::DiskConfirm => self.confirm.handle_input(key),
            Step::BootMode => {
                self.config.uefi = std::path::Path::new("/sys/firmware/efi").is_dir();
                self.boot_mode.handle_input(key)
            }
            Step::Filesystem => {
                let r = self.filesystem.handle_input(key);
                if r == StepResult::Next {
                    self.config.fs_type = self.filesystem.selected();
                }
                r
            }
            Step::Hostname => {
                let r = self.hostname.handle_input(key);
                if r == StepResult::Next {
                    self.config.hostname = self.hostname.value();
                }
                r
            }
            Step::RootPassword => {
                let r = self.root_password.handle_input(key);
                if r == StepResult::Next {
                    self.config.root_password = self.root_password.password();
                }
                r
            }
            Step::UserAccount => {
                let r = self.user_account.handle_input(key);
                if r == StepResult::Next {
                    self.config.username = self.user_account.username();
                    self.config.user_password = self.user_account.password();
                }
                r
            }
            Step::Timezone => {
                let r = self.timezone.handle_input(key);
                if r == StepResult::Next {
                    self.config.timezone = self.timezone.selected_timezone();
                }
                r
            }
            Step::Summary => {
                let r = self.summary.handle_input(key);
                if r == StepResult::Next {
                    self.start_install();
                }
                r
            }
            Step::Installing => {
                self.progress.handle_input(key)
            }
            Step::Complete => {
                return self.complete.handle_input(key) == StepResult::Quit;
            }
            Step::Error => {
                return self.error.handle_input(key) == StepResult::Quit;
            }
        };

        match result {
            StepResult::Next => {
                if self.step_idx + 1 < STEP_ORDER.len() {
                    self.step_idx += 1;
                }
            }
            StepResult::Back => {
                if self.step_idx > 0 {
                    self.step_idx -= 1;
                }
            }
            StepResult::Quit => return true,
            StepResult::Continue => {}
        }

        false
    }

    /// Poll the install progress channel (non-blocking).
    /// Called from the main event loop each tick.
    pub fn poll_progress(&mut self) {
        if self.current_step() != Step::Installing {
            return;
        }
        if let Some(rx) = &self.install_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Progress::Update { percent, message } => {
                        self.progress.update(percent, &message);
                    }
                    Progress::Complete => {
                        self.progress.update(100, "Installation complete!");
                        self.install_rx = None;
                        self.step_idx = STEP_ORDER
                            .iter()
                            .position(|s| *s == Step::Complete)
                            .unwrap();
                        break;
                    }
                    Progress::Error(msg) => {
                        self.error.set_message(&msg);
                        self.install_rx = None;
                        self.step_idx = STEP_ORDER
                            .iter()
                            .position(|s| *s == Step::Error)
                            .unwrap_or(self.step_idx);
                        break;
                    }
                }
            }
        }
    }

    fn start_install(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.install_rx = Some(rx);
        let config = self.config.clone();
        std::thread::spawn(move || {
            install::run_install(config, tx);
        });
    }
}
