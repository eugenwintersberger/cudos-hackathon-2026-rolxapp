//! Application configuration.
//!
//! The configuration is a TOML file named `config.toml`. It is searched in
//! `/etc/rolx/tracker` first and then in `$HOME/.config/rolx/tracker`; the
//! first file found is used. Without a file the defaults apply.

use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use serde::Deserialize;

const SYSTEM_DIR: &str = "/etc/rolx/tracker";
const FILE_NAME: &str = "config.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Argon2 PHC string (`$argon2id$...`) of the administrator password.
    /// Create one with `tracker --hash-password`.
    pub admin_password_hash: Option<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, toml::de::Error),
    Hash(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "cannot read {}: {e}", path.display()),
            Self::Parse(path, e) => write!(f, "invalid config {}: {e}", path.display()),
            Self::Hash(e) => write!(f, "cannot hash password: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Candidate configuration files in order of precedence.
pub fn search_paths() -> Vec<PathBuf> {
    let mut paths = vec![Path::new(SYSTEM_DIR).join(FILE_NAME)];
    if let Some(home) = env::var_os("HOME") {
        paths.push(Path::new(&home).join(".config/rolx/tracker").join(FILE_NAME));
    }
    paths
}

impl Config {
    /// Load the first configuration file found, or the defaults if none exists.
    pub fn load() -> Result<Self, ConfigError> {
        for path in search_paths() {
            match fs::read_to_string(&path) {
                Ok(text) => return Self::parse(&text).map_err(|e| ConfigError::Parse(path, e)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(ConfigError::Io(path, e)),
            }
        }
        Ok(Self::default())
    }

    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    /// Whether an administrator password has been configured.
    pub fn has_admin_password(&self) -> bool {
        self.admin_password_hash.is_some()
    }

    /// Check `password` against the configured administrator password.
    /// Returns `false` if none is configured or the stored hash is invalid.
    pub fn verify_admin_password(&self, password: &str) -> bool {
        let Some(hash) = self.admin_password_hash.as_deref() else {
            return false;
        };
        let Ok(hash) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    }
}

/// Hash a password for use as `admin_password_hash`.
pub fn hash_password(password: &str) -> Result<String, ConfigError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| ConfigError::Hash(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_admin_password() {
        let hash = hash_password("secret").unwrap();
        let config = Config::parse(&format!("admin_password_hash = \"{hash}\"")).unwrap();
        assert!(config.has_admin_password());
        assert!(config.verify_admin_password("secret"));
        assert!(!config.verify_admin_password("wrong"));
        assert!(!config.verify_admin_password(""));
    }

    #[test]
    fn no_password_configured_rejects_everything() {
        let config = Config::default();
        assert!(!config.has_admin_password());
        assert!(!config.verify_admin_password(""));
    }

    #[test]
    fn invalid_hash_rejects() {
        let config = Config::parse("admin_password_hash = \"nonsense\"").unwrap();
        assert!(!config.verify_admin_password("nonsense"));
    }

    #[test]
    fn unknown_keys_are_errors() {
        assert!(Config::parse("admin_pasword_hash = \"x\"").is_err());
    }
}
