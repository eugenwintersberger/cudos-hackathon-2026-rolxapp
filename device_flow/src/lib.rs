//! Sign-in to Rolx via Google OAuth 2.0, session handling and configuration.
//!
//! The library never prints; callers decide how to present results.
//! Login is two-phase: [`login_code`] / [`login_browser`] start it and return a pending
//! login, whose `finish()` completes it. Commands: login, [`logout`], [`whoami`], [`commands::config`].

pub mod commands;
pub mod config;
pub mod error;
pub mod logging;
mod oauth2;
pub mod rolx;
pub mod session;

pub use commands::login::{PendingBrowserLogin, PendingCodeLogin, login_browser, login_code};
pub use commands::logout::logout;
pub use commands::whoami::whoami;
pub use config::{AppConfig, ClientCredentials};
pub use error::{Error, Result};
pub use oauth2::device_flow::DeviceCodePrompt;
pub use session::{BearerToken, Session};
