//! The work session that runs between "Start work" and "Stop work".
//!
//! Only the start timestamp is stored, as RFC 3339 text in a small file
//! (readable only by its owner). Without a file path the session is kept in
//! memory, so the tracker also works where there is nothing to write to.

use std::{
    fmt, fs, io,
    io::Write as _,
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{DateTime, Local, NaiveDate, Timelike};

/// A work session that has been started but not recorded at Rolx yet.
#[derive(Debug, Clone)]
pub struct WorkSession {
    /// When the user pressed "Start work".
    pub started_at: DateTime<Local>,
}

impl WorkSession {
    /// A session that starts now.
    pub fn start() -> Self {
        Self {
            started_at: Local::now(),
        }
    }

    /// The day the entry is recorded on: the day the work started.
    pub fn date(&self) -> NaiveDate {
        self.started_at.date_naive()
    }

    /// Whether the session started on the current day.
    pub fn is_today(&self) -> bool {
        self.date() == Local::now().date_naive()
    }

    /// Seconds since local midnight: the `begin` of the record entry.
    pub fn begin_seconds(&self) -> i64 {
        begin_seconds(self.started_at)
    }

    /// Seconds from the start to `end`.
    pub fn duration_seconds(&self, end: DateTime<Local>) -> i64 {
        duration_seconds(self.started_at, end)
    }

    /// Whole seconds since the start; zero if the clock went backwards.
    pub fn elapsed_seconds(&self) -> i64 {
        (Local::now() - self.started_at).num_seconds().max(0)
    }
}

/// Seconds since local midnight of `start`.
pub fn begin_seconds(start: DateTime<Local>) -> i64 {
    (start.hour() as i64) * 3600 + (start.minute() as i64) * 60 + start.second() as i64
}

/// Seconds from `start` to `end`, at least one: Rolx drops entries of length
/// zero.
pub fn duration_seconds(start: DateTime<Local>, end: DateTime<Local>) -> i64 {
    (end - start).num_seconds().max(1)
}

/// `seconds` as `1h 12m`, or as `45m` below one hour.
pub fn format_duration(seconds: i64) -> String {
    let minutes = (seconds + 30) / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[derive(Debug)]
pub enum WorkError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, chrono::ParseError),
}

impl fmt::Display for WorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "cannot access {}: {e}", path.display()),
            Self::Parse(path, e) => write!(f, "invalid work session in {}: {e}", path.display()),
        }
    }
}

impl std::error::Error for WorkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(_, e) => Some(e),
            Self::Parse(_, e) => Some(e),
        }
    }
}

/// Stores the start timestamp of the running session.
pub struct Store {
    path: Option<PathBuf>,
    memory: Mutex<Option<WorkSession>>,
}

impl Store {
    /// A store that keeps the session in `path`, or only in memory with `None`.
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            memory: Mutex::default(),
        }
    }

    /// The stored session, if any.
    pub fn load(&self) -> Result<Option<WorkSession>, WorkError> {
        let Some(path) = &self.path else {
            return Ok(self.memory.lock().unwrap().clone());
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(WorkError::Io(path.clone(), e)),
        };
        let started_at = text
            .trim()
            .parse::<DateTime<Local>>()
            .map_err(|e| WorkError::Parse(path.clone(), e))?;
        Ok(Some(WorkSession { started_at }))
    }

    /// Replaces the stored session.
    pub fn save(&self, session: &WorkSession) -> Result<(), WorkError> {
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| WorkError::Io(parent.to_path_buf(), e))?;
            }
            // Write to a temporary file first, so a crash never leaves a
            // truncated session.
            let tmp = path.with_extension("json.tmp");
            let text = session.started_at.to_rfc3339();
            write_private(&tmp, text.as_bytes()).map_err(|e| WorkError::Io(tmp.clone(), e))?;
            fs::rename(&tmp, path).map_err(|e| WorkError::Io(path.clone(), e))?;
        }
        *self.memory.lock().unwrap() = Some(session.clone());
        Ok(())
    }

    /// Removes the stored session. Succeeds if there is none.
    pub fn clear(&self) -> Result<(), WorkError> {
        *self.memory.lock().unwrap() = None;
        let Some(path) = &self.path else {
            return Ok(());
        };
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WorkError::Io(path.clone(), e)),
        }
    }
}

#[cfg(unix)]
fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(data)
}

#[cfg(not(unix))]
fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    fs::write(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32, minute: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 31, hour, minute, 7).unwrap()
    }

    #[test]
    fn begin_is_seconds_since_midnight() {
        assert_eq!(begin_seconds(at(8, 5)), 8 * 3600 + 5 * 60 + 7);
        assert_eq!(begin_seconds(at(0, 0)), 7);
        // Rolx counts seconds: 11:46:11 is 42371 seconds after midnight.
        assert_eq!(begin_seconds(at(11, 46)), 11 * 3600 + 46 * 60 + 7);
    }

    #[test]
    fn duration_is_in_seconds() {
        let start = at(8, 5);
        assert_eq!(duration_seconds(start, at(9, 17)), 72 * 60);
        assert_eq!(
            duration_seconds(start, start + chrono::Duration::seconds(4289)),
            4289
        );
        // Rolx drops entries without a duration
        assert_eq!(duration_seconds(start, start), 1);
        assert_eq!(
            duration_seconds(start, start - chrono::Duration::minutes(5)),
            1
        );
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(45 * 60), "45m");
        assert_eq!(format_duration(72 * 60), "1h 12m");
        assert_eq!(format_duration(120 * 60), "2h 0m");
        // rounded to the nearest minute
        assert_eq!(format_duration(4289), "1h 11m");
        assert_eq!(format_duration(4290), "1h 12m");
    }

    #[test]
    fn session_belongs_to_the_start_day() {
        let session = WorkSession {
            started_at: at(23, 30),
        };
        assert_eq!(session.date(), NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
        assert_eq!(session.begin_seconds(), 23 * 3600 + 30 * 60 + 7);
        assert_eq!(session.duration_seconds(at(23, 45)), 15 * 60);
    }

    #[test]
    fn only_a_session_from_today_is_today() {
        assert!(WorkSession::start().is_today());
        let old = Local.with_ymd_and_hms(2000, 1, 2, 3, 4, 5).unwrap();
        assert!(!WorkSession { started_at: old }.is_today());
    }

    #[test]
    fn stores_and_clears_the_session() {
        let dir = std::env::temp_dir().join(format!("rolx-work-{}", std::process::id()));
        let path = dir.join("work-session.json");
        let store = Store::new(Some(path.clone()));
        assert!(store.load().unwrap().is_none());

        let session = WorkSession {
            started_at: at(8, 5),
        };
        store.save(&session).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(
            loaded.started_at.to_rfc3339(),
            session.started_at.to_rfc3339()
        );
        assert!(path.exists());

        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
        assert!(!path.exists());
        // clearing twice is fine
        store.clear().unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_file_is_an_error() {
        let dir = std::env::temp_dir().join(format!("rolx-work-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("work-session.json");
        fs::write(&path, "nonsense").unwrap();
        let store = Store::new(Some(path));
        assert!(store.load().is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn works_without_a_file() {
        let store = Store::new(None);
        assert!(store.load().unwrap().is_none());
        store.save(&WorkSession::start()).unwrap();
        assert!(store.load().unwrap().is_some());
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }
}
