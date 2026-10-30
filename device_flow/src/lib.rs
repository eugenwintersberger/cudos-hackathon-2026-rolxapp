//! Sign-in to Rolx via Google OAuth 2.0, session handling and configuration.
//!
//! The library never prints; callers decide how to present results.
//! Commands: [`login`], [`logout`], [`whoami`] and [`config`].

pub mod commands;
pub mod config;
pub mod error;
pub mod logging;
mod oauth2;
pub mod rolx;
pub mod session;

pub use commands::login::{AuthFlow, login};
pub use commands::logout::logout;
pub use commands::whoami::whoami;
pub use config::{AppConfig, ClientCredentials};
pub use error::{Error, Result};
pub use session::{BearerToken, Session};
