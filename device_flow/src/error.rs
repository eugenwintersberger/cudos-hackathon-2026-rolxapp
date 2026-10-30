use thiserror::Error;

use crate::oauth2::{device_flow, native_app_flow};
use crate::rolx::sign_in::SignInError;
use crate::session::{DeletionError, LoadError, SaveError};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("config error: {0}")]
    Config(String),

    #[error("`{0}` config missing — add it to the config")]
    MissingClientConfig(&'static str),

    #[error("id_token missing in {0} response")]
    MissingIdToken(&'static str),

    #[error("native app flow failed: {0}")]
    NativeAppFlow(#[from] native_app_flow::Error),

    #[error("device flow failed: {0}")]
    DeviceFlow(#[from] device_flow::Error),

    #[error("sign-in failed: {0}")]
    SignIn(#[from] SignInError),

    #[error("cannot load session: {0}")]
    SessionLoad(#[from] LoadError),

    #[error("cannot save session: {0}")]
    SessionSave(#[from] SaveError),

    #[error("cannot delete session: {0}")]
    SessionDelete(#[from] DeletionError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}
