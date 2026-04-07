mod config;
mod install;
mod log;
mod ui;
mod wizard;

use std::io;
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use config::{InstallConfig, LOG_PATH, SQUASHFS_CANDIDATES};
use wizard::Wizard;

static SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);

fn main() {
    // Root check
    if !nix::unistd::getuid().is_root() {
        eprintln!("ERROR: eclipse-install must be run as root.");
        eprintln!("  Run: sudo eclipse-install");
        process::exit(1);
    }

    // Locate squashfs source
    let squashfs_mount = match find_squashfs_source() {
        Some(path) => path,
        None => {
            eprintln!("ERROR: Cannot find Eclipse Linux root filesystem to install from.");
            eprintln!("  This installer is meant to run from the live ISO.");
            process::exit(1);
        }
    };

    // Initialize logger
    if let Err(e) = log::Logger::init(LOG_PATH) {
        eprintln!("WARNING: Could not open log file {}: {}", LOG_PATH, e);
    }
    log::log("Eclipse Linux Installer started");
    log::log(&format!("Source: {}", squashfs_mount));

    // Install signal handlers
    install_signal_handlers();

    // Run the TUI
    let config = InstallConfig::new(squashfs_mount);
    let mut wizard = Wizard::new(config);

    let exit_result = run_tui(&mut wizard);

    // Check if the user chose to reboot
    let should_reboot = wizard.complete.should_reboot();

    if let Err(e) = exit_result {
        eprintln!("TUI error: {}", e);
        process::exit(1);
    }

    if should_reboot {
        log::log("Rebooting...");
        let _ = process::Command::new("dynamodctl")
            .args(["shutdown", "reboot"])
            .status();
        let _ = process::Command::new("reboot").status();
    }
}

fn find_squashfs_source() -> Option<String> {
    for candidate in SQUASHFS_CANDIDATES {
        let init_path = format!("{}/sbin/dynamod-init", candidate);
        if Path::new(candidate).is_dir() && Path::new(&init_path).is_file() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn install_signal_handlers() {
    unsafe {
        // SIGINT (Ctrl+C) and SIGTERM
        for sig in &[nix::sys::signal::Signal::SIGINT, nix::sys::signal::Signal::SIGTERM] {
            let handler = nix::sys::signal::SigHandler::Handler(signal_handler);
            let action = nix::sys::signal::SigAction::new(
                handler,
                nix::sys::signal::SaFlags::empty(),
                nix::sys::signal::SigSet::empty(),
            );
            let _ = nix::sys::signal::sigaction(*sig, &action);
        }
    }
}

extern "C" fn signal_handler(_sig: nix::libc::c_int) {
    SIGNAL_RECEIVED.store(true, Ordering::SeqCst);
}

fn run_tui(wizard: &mut Wizard) -> io::Result<()> {
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = event_loop(&mut terminal, wizard);

    // Always restore terminal state
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    wizard: &mut Wizard,
) -> io::Result<()> {
    loop {
        // Check for signal
        if SIGNAL_RECEIVED.load(Ordering::SeqCst) {
            log::log("Signal received, cleaning up...");
            install::emergency_cleanup();
            return Ok(());
        }

        // Poll install progress (non-blocking)
        wizard.poll_progress();

        // Draw
        terminal.draw(|frame| {
            wizard.draw(frame, frame.area());
        })?;

        // Poll for events with a short timeout so we can check progress
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events, ignore release/repeat
                if key.kind == KeyEventKind::Press {
                    if wizard.handle_input(key) {
                        return Ok(());
                    }
                }
            }
        }
    }
}
