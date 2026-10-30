//! Client for the Rolx service.
//!
//! The Google ID token from [`crate::oidc`] is exchanged for a Rolx bearer token.

use std::fmt;

use serde::Deserialize;
use ureq::Agent;

#[derive(Debug)]
pub enum Error {
    /// The request could not be sent or the response not be read.
    Transport(String),
    /// The server rejected the request.
    Server(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "request failed: {e}"),
            Self::Server(e) => write!(f, "server error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Self::Transport(e.to_string())
    }
}

fn agent() -> Agent {
    // The caller inspects the status code itself.
    Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

/// The session granted by the Rolx server.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub bearer_token: String,
    /// Expiry time as sent by the server (ISO 8601).
    pub expires: String,
}

impl fmt::Debug for Approval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Approval")
            .field("bearer_token", &"<redacted>")
            .field("expires", &self.expires)
            .finish()
    }
}

/// The Rolx service.
pub struct RolxClient {
    server_url: String,
    agent: Agent,
}

impl RolxClient {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into().trim_end_matches('/').to_owned(),
            agent: agent(),
        }
    }

    /// Exchange a Google ID token for a Rolx bearer token.
    pub fn sign_in(&self, id_token: &str) -> Result<Approval, Error> {
        let mut response = self
            .agent
            .post(format!("{}/api/v1/SignIn", self.server_url))
            .send_json(serde_json::json!({ "googleIdToken": id_token }))?;
        if !response.status().is_success() {
            return Err(Error::Server(format!("sign-in failed ({})", response.status())));
        }
        Ok(response.body_mut().read_json()?)
    }
}

