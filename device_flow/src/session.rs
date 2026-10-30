use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearerToken(String);

impl BearerToken {
    pub fn from_string(s: String) -> Self {
        BearerToken(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub bearer_token: BearerToken,
    pub expires: DateTime<Utc>,
    pub user_id: String,
    pub user_email: String,
    pub user_first_name: String,
    pub user_last_name: String,
}

impl Session {
    pub fn is_near_expiry(&self) -> bool {
        self.expires < Utc::now() + Duration::minutes(10)
    }

    pub fn is_expired(&self) -> bool {
        self.expires < Utc::now()
    }
}

/// Persists the session between runs. Implement it for devices without a filesystem;
/// [`FileSessionStore`] is the default (feature `file-store`).
pub trait SessionStore {
    /// Returns the stored session, or `None` if there is none.
    fn load(&self) -> Result<Option<Session>>;

    /// Stores the session, replacing any previous one.
    fn save(&self, session: &Session) -> Result<()>;

    /// Removes the stored session. Succeeds if there is none.
    fn delete(&self) -> Result<()>;
}

/// Stores the session as JSON in a file (mode `0600` on Unix).
#[cfg(feature = "file-store")]
#[derive(Debug, Clone)]
pub struct FileSessionStore {
    path: std::path::PathBuf,
}

#[cfg(feature = "file-store")]
impl FileSessionStore {
    /// Stores the session at the given file path, e.g. on a writable partition of an embedded device.
    pub fn at(path: impl Into<std::path::PathBuf>) -> Self {
        FileSessionStore { path: path.into() }
    }

    /// Stores the session at `<state_dir>/rolx/session.json` (falling back to the local data
    /// directory where there is no state directory).
    pub fn default_location() -> Result<Self> {
        let base = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .ok_or_else(|| {
                crate::error::Error::Config("cannot determine state directory".into())
            })?;
        Ok(Self::at(base.join("rolx").join("session.json")))
    }
}

#[cfg(feature = "file-store")]
impl SessionStore for FileSessionStore {
    fn load(&self) -> Result<Option<Session>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, session: &Session) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(session)?;
        // Write to a temporary file first, so a crash never leaves a truncated session.
        let tmp = self.path.with_extension("json.tmp");
        write_private(&tmp, text.as_bytes())?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(all(feature = "file-store", unix))]
fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(data)
}

#[cfg(all(feature = "file-store", not(unix)))]
fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, data)
}

#[cfg(all(test, feature = "file-store"))]
#[path = "session.tests.rs"]
mod tests;
