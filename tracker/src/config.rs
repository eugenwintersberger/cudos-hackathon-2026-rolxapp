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

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Argon2 PHC string (`$argon2id$...`) of the administrator password.
    /// Create one with `tracker --hash-password`.
    pub admin_password_hash: Option<String>,
    /// Base URL of the Rolx service.
    pub server_url: Option<String>,
    /// OIDC client used for the device flow login.
    pub device_client_id: Option<String>,
    pub device_client_secret: Option<String>,
    /// Directory of the log files (default `/var/log`).
    pub log_dir: Option<String>,
    /// File that holds the start timestamp of the running work session
    /// (default `$HOME/.config/rolx/tracker/work-session.json`).
    pub work_session_path: Option<String>,
    /// wpa_supplicant configuration that the WLAN dialog updates
    /// (default `/etc/wpa_supplicant.conf`).
    pub wlan_config_path: Option<String>,
    /// Program and arguments that restart the network so that the new
    /// configuration takes effect (default `["/etc/init.d/S40network",
    /// "restart"]`).
    pub wlan_restart_command: Option<Vec<String>>,
}

const DEFAULT_LOG_DIR: &str = "/var/log";
const DEFAULT_WLAN_CONFIG_PATH: &str = "/etc/wpa_supplicant.conf";
const USER_DIR: &str = ".config/rolx/tracker";
const WORK_SESSION_FILE: &str = "work-session.json";

/// Everything needed to sign in at the Rolx service.
pub struct LoginConfig<'a> {
    pub server_url: &'a str,
    pub client_id: &'a str,
    pub client_secret: &'a str,
}

// Written by hand so that secrets never end up in logs.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("admin_password_hash", &self.admin_password_hash.as_ref().map(|_| "<set>"))
            .field("server_url", &self.server_url)
            .field("device_client_id", &self.device_client_id)
            .field("device_client_secret", &self.device_client_secret.as_ref().map(|_| "<set>"))
            .field("log_dir", &self.log_dir)
            .field("work_session_path", &self.work_session_path)
            .field("wlan_config_path", &self.wlan_config_path)
            .field("wlan_restart_command", &self.wlan_restart_command)
            .finish()
    }
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

    /// The login settings, if all of them are configured.
    pub fn login(&self) -> Option<LoginConfig<'_>> {
        Some(LoginConfig {
            server_url: self.server_url.as_deref()?,
            client_id: self.device_client_id.as_deref()?,
            client_secret: self.device_client_secret.as_deref()?,
        })
    }

    pub fn log_dir(&self) -> &str {
        self.log_dir.as_deref().unwrap_or(DEFAULT_LOG_DIR)
    }

    /// Where the running work session is stored. `None` if there is no home
    /// directory to write to; the session then only lives in memory.
    pub fn work_session_path(&self) -> Option<PathBuf> {
        if let Some(path) = &self.work_session_path {
            return Some(PathBuf::from(path));
        }
        let home = env::var_os("HOME")?;
        Some(Path::new(&home).join(USER_DIR).join(WORK_SESSION_FILE))
    }

    pub fn wlan_config_path(&self) -> &str {
        self.wlan_config_path.as_deref().unwrap_or(DEFAULT_WLAN_CONFIG_PATH)
    }

    pub fn wlan_restart_command(&self) -> Vec<String> {
        self.wlan_restart_command
            .clone()
            .unwrap_or_else(|| vec!["/etc/init.d/S40network".into(), "restart".into()])
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
    fn login_needs_all_three_settings() {
        let all = Config::parse(
            "server_url = \"https://x\"\ndevice_client_id = \"i\"\ndevice_client_secret = \"s\"",
        )
        .unwrap();
        assert!(all.login().is_some());
        let partial = Config::parse("server_url = \"https://x\"").unwrap();
        assert!(partial.login().is_none());
        assert!(!format!("{all:?}").contains("\"s\""));
    }

    #[test]
    fn wlan_settings_have_defaults() {
        let config = Config::default();
        assert_eq!(config.wlan_config_path(), "/etc/wpa_supplicant.conf");
        assert_eq!(config.wlan_restart_command(), ["/etc/init.d/S40network", "restart"]);

        let config = Config::parse(
            "wlan_config_path = \"/tmp/w.conf\"\nwlan_restart_command = [\"systemctl\", \"restart\", \"wpa_supplicant\"]",
        )
        .unwrap();
        assert_eq!(config.wlan_config_path(), "/tmp/w.conf");
        assert_eq!(config.wlan_restart_command().len(), 3);
    }

    #[test]
    fn unknown_keys_are_errors() {
        assert!(Config::parse("admin_pasword_hash = \"x\"").is_err());
    }

    #[test]
    fn work_session_path_defaults_to_the_user_config() {
        let config = Config::parse("work_session_path = \"/tmp/w.json\"").unwrap();
        assert_eq!(config.work_session_path(), Some(PathBuf::from("/tmp/w.json")));

        let default = Config::default().work_session_path();
        if let Some(path) = default {
            assert!(path.ends_with(".config/rolx/tracker/work-session.json"), "{path:?}");
        }
    }
}
