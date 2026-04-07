use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::process::Output;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub struct Logger {
    file: Mutex<BufWriter<File>>,
}

impl Logger {
    pub fn init(path: &str) -> std::io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let logger = Logger {
            file: Mutex::new(BufWriter::new(file)),
        };
        LOGGER.get_or_init(|| logger);
        Ok(())
    }

    fn timestamp() -> String {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let h = (secs % 86400) / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }
}

pub fn log(msg: &str) {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut f) = logger.file.lock() {
            let _ = writeln!(f, "[{}] {}", Logger::timestamp(), msg);
            let _ = f.flush();
        }
    }
}

/// Log stdout and stderr from a child process.
pub fn log_output(cmd: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        log(&format!("[{}] stdout: {}", cmd, stdout.trim_end()));
    }
    if !stderr.is_empty() {
        log(&format!("[{}] stderr: {}", cmd, stderr.trim_end()));
    }
}
