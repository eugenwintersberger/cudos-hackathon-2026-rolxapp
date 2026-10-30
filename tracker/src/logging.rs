//! Logging with `tracing` into size-rotated files.
//!
//! The log is written to `rolx-tracker.log` in the log directory. When the
//! file has reached [`MAX_FILE_SIZE`] it becomes `rolx-tracker.log.1`, the
//! former `.1` becomes `.2` and so on. At most [`MAX_FILES`] files are kept,
//! the oldest one is deleted. The verbosity is set with `RUST_LOG` (default:
//! `info`).

use std::{
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

use file_rotate::{ContentLimit, FileRotate, compression::Compression, suffix::AppendCount};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt};

/// Size at which the log file is rotated.
pub const MAX_FILE_SIZE: usize = 20 * 1024 * 1024;
/// Number of log files kept, including the current one.
pub const MAX_FILES: usize = 10;

const FILE_NAME: &str = "rolx-tracker.log";

/// Keeps the log writer alive; dropping it flushes the pending messages.
pub struct Guard(#[allow(dead_code)] WorkerGuard);

/// A rotating log file in `dir`.
fn rotating_file(dir: &Path, max_size: usize) -> std::io::Result<FileRotate<AppendCount>> {
    fs::create_dir_all(dir)?;
    let mut options = OpenOptions::new();
    options.read(true).create(true).append(true).mode(0o640);

    // Opening the file now reports a missing permission right away instead
    // of at the first log message.
    options.open(dir.join(FILE_NAME))?;

    Ok(FileRotate::new(
        dir.join(FILE_NAME),
        // rotated files do not count as the current one
        AppendCount::new(MAX_FILES - 1),
        // rotates after the message that exceeded the limit, so that no
        // message is split between two files
        ContentLimit::BytesSurpassed(max_size),
        Compression::None,
        Some(options),
    ))
}

/// Install the global subscriber. Falls back to stderr if the log file
/// cannot be opened, e.g. when running as a normal user on a desktop.
pub fn init(dir: &Path) -> Guard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    match rotating_file(dir, MAX_FILE_SIZE) {
        Ok(file) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
            fmt()
                .with_env_filter(filter)
                .with_writer(writer)
                .with_ansi(false)
                .init();
            Guard(guard)
        }
        Err(e) => {
            let (writer, guard) = tracing_appender::non_blocking(std::io::stderr());
            fmt().with_env_filter(filter).with_writer(writer).init();
            tracing::warn!("cannot log to {}: {e}; logging to stderr", dir.display());
            Guard(guard)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn rotates_and_keeps_at_most_ten_files() {
        let dir = std::env::temp_dir().join(format!("rolx-log-{}", std::process::id()));
        let mut file = rotating_file(&dir, 1024).unwrap();

        let chunk = vec![b'x'; 600];
        for _ in 0..100 {
            file.write_all(&chunk).unwrap();
        }
        file.flush().unwrap();

        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names.len(), MAX_FILES, "{names:?}");
        assert!(names.contains(&"rolx-tracker.log".to_owned()));
        assert!(names.contains(&"rolx-tracker.log.9".to_owned()));
        fs::remove_dir_all(&dir).unwrap();
    }
}
