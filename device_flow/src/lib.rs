//! Sign-in to Rolx via Google OAuth 2.0, session handling and configuration.
//!
//! The library never prints; callers decide how to present results.
//! Login is two-phase: [`login_code`] / [`login_browser`] start it and return a pending
//! login, whose `finish()` completes it and stores the session in a [`SessionStore`].
//!
//! ```no_run
//! use device_flow::{AppConfig, SessionStore};
//!
//! async fn sign_in(cfg: &AppConfig, store: &impl SessionStore) -> device_flow::Result<()> {
//!     let pending = device_flow::login_code(cfg).await?;
//!     let prompt = pending.prompt();
//!     println!("Visit {} and enter {}", prompt.verification_uri, prompt.user_code);
//!     let session = pending.finish(store).await?;
//!     println!("Hello {}", session.user_first_name);
//!     Ok(())
//! }
//! ```
//!
//! # Cargo features
//! - `file-store` (default): [`FileSessionStore`], `AppConfig::load` and `config_path`.
//! - `browser-login` (default): `login_browser` and `PendingBrowserLogin`.
//! - `uncensored_logs`: don't redact secrets in debug logs.

mod commands;
mod config;
mod error;
mod logging;
mod oauth2;
mod rolx;
mod session;

pub use commands::extend::extend_session;
#[cfg(feature = "browser-login")]
pub use commands::login::{PendingBrowserLogin, login_browser};
pub use commands::login::{PendingCodeLogin, login_code};
pub use commands::logout::logout;
pub use commands::whoami::whoami;
#[cfg(feature = "file-store")]
pub use config::config_path;
pub use config::{AppConfig, ClientCredentials};
pub use error::{Error, Result};
pub use oauth2::device_flow::DeviceCodePrompt;
#[cfg(feature = "file-store")]
pub use session::FileSessionStore;
pub use session::{BearerToken, Session, SessionStore};
