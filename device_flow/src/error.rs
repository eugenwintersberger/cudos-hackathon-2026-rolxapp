use std::path::PathBuf;

use thiserror::Error;

use crate::oauth2::device_flow;
#[cfg(feature = "browser-login")]
use crate::oauth2::native_app_flow;
use crate::rolx::sign_in::{SignInError, SignInExtensionError};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("config file {} not found — create it (see `config path`)", .0.display())]
    ConfigNotFound(PathBuf),

    #[error("config error: {0}")]
    Config(String),

    #[error("`{0}` config missing — add it to the config")]
    MissingClientConfig(&'static str),

    #[error("id_token missing in {0} response")]
    MissingIdToken(&'static str),

    /// The user denied the authorization request.
    #[error("access denied")]
    AccessDenied,

    /// The device code expired before the user authorized the device.
    #[error("device authorization expired")]
    Expired,

    /// Rolx did not accept the Google ID token.
    #[error("server did not authorize the ID token — is your account confirmed?")]
    Unauthorized,

    /// There is no stored session: log in first.
    #[error("no session found")]
    NoSession,

    /// The stored session was rejected by Rolx: log in again.
    #[error("session expired")]
    SessionExpired,

    #[cfg(feature = "browser-login")]
    #[error("native app flow failed: {0}")]
    NativeAppFlow(#[from] native_app_flow::Error),

    #[error("device flow failed: {0}")]
    DeviceFlow(device_flow::Error),

    #[error("sign-in failed: {0}")]
    SignIn(SignInError),

    #[error("session extension failed: {0}")]
    SignInExtension(SignInExtensionError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg(feature = "file-store")]
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl From<device_flow::Error> for Error {
    fn from(e: device_flow::Error) -> Self {
        match e {
            device_flow::Error::AccessDenied => Error::AccessDenied,
            device_flow::Error::Expired => Error::Expired,
            other => Error::DeviceFlow(other),
        }
    }
}

impl From<SignInError> for Error {
    fn from(e: SignInError) -> Self {
        match e {
            SignInError::Unauthorized => Error::Unauthorized,
            other => Error::SignIn(other),
        }
    }
}

impl From<SignInExtensionError> for Error {
    fn from(e: SignInExtensionError) -> Self {
        match e {
            SignInExtensionError::SessionExpired => Error::SessionExpired,
            other => Error::SignInExtension(other),
        }
    }
}
