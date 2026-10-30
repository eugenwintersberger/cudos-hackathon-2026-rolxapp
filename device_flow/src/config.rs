use serde::{Deserialize, Serialize};

#[cfg(feature = "file-store")]
use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCredentials {
    pub id: String,
    pub secret: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server_url: String,
    pub google_desktop_client: Option<ClientCredentials>,
    pub google_device_client: Option<ClientCredentials>,
}

impl AppConfig {
    /// Reads the config file at [`config_path`].
    #[cfg(feature = "file-store")]
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::ConfigNotFound(path));
            }
            Err(e) => return Err(e.into()),
        };
        Ok(toml::from_str(&text)?)
    }

    /// Returns the config with `server_url` replaced, if `server` is given.
    pub fn with_server_override(mut self, server: Option<String>) -> Self {
        if let Some(server) = server {
            self.server_url = server;
        }
        self
    }
}

/// Returns the config file path `<config_dir>/rolx/config.toml`. The file need not exist.
#[cfg(feature = "file-store")]
pub fn config_path() -> Result<std::path::PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| Error::Config("cannot determine config directory".into()))?;
    Ok(base.join("rolx").join("config.toml"))
}
