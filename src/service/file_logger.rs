use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_FILES: u32 = 3;

/// A rotating file logger. Writes log lines to a file, rotating when
/// the file exceeds MAX_FILE_SIZE. Keeps up to MAX_FILES rotated copies.
pub struct FileLogger {
    inner: Mutex<FileLoggerInner>,
}

struct FileLoggerInner {
    path: PathBuf,
    file: Option<File>,
    bytes_written: u64,
}

impl FileLogger {
    pub fn new() -> Option<Self> {
        let mut path = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        path.push("govee2mqtt.log");

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;

        let bytes_written = file.metadata().map(|m| m.len()).unwrap_or(0);

        Some(Self {
            inner: Mutex::new(FileLoggerInner {
                path,
                file: Some(file),
                bytes_written,
            }),
        })
    }

    pub fn write_line(&self, line: &str) {
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let Some(file) = inner.file.as_mut() else {
            return;
        };

        let bytes = line.as_bytes();
        if file.write_all(bytes).is_err() || file.write_all(b"\n").is_err() {
            return;
        }
        inner.bytes_written += bytes.len() as u64 + 1;

        if inner.bytes_written >= MAX_FILE_SIZE {
            self.rotate(&mut inner);
        }
    }

    fn rotate(&self, inner: &mut FileLoggerInner) {
        // Close current file
        inner.file.take();

        // Rotate: .log.2 -> .log.3, .log.1 -> .log.2, .log -> .log.1
        for i in (1..MAX_FILES).rev() {
            let from = inner.path.with_extension(format!("log.{i}"));
            let to = inner.path.with_extension(format!("log.{}", i + 1));
            let _ = fs::rename(&from, &to);
        }
        let rotated = inner.path.with_extension("log.1");
        let _ = fs::rename(&inner.path, &rotated);

        // Open new file
        inner.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inner.path)
            .ok();
        inner.bytes_written = 0;
    }
}

static FILE_LOGGER: std::sync::OnceLock<FileLogger> = std::sync::OnceLock::new();

/// Initialize the file logger. Call once at startup.
pub fn init() {
    if let Some(logger) = FileLogger::new() {
        FILE_LOGGER.set(logger).ok();
    }
}

/// Write a log line to the file. No-op if file logging isn't initialized.
pub fn write_line(line: &str) {
    if let Some(logger) = FILE_LOGGER.get() {
        logger.write_line(line);
    }
}
