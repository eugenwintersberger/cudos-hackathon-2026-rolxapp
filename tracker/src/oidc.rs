//! OIDC device flow against Google (RFC 8628).
//!
//! The flow yields a Google ID token, which the Rolx server exchanges for its
//! own bearer token (see [`crate::rolx::RolxClient::sign_in`]).

use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use ureq::Agent;

const DEVICE_CODE_URL: &str = "https://oauth2.googleapis.com/device/code";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
// `profile` is required: the server reads the user's name from the token.
const SCOPE: &str = "openid email profile";

#[derive(Debug)]
pub enum Error {
    /// The request could not be sent or the response not be read.
    Transport(String),
    /// The server rejected the request.
    Server(String),
    /// The user refused access.
    Denied,
    /// The device code expired before the user finished.
    Expired,
    /// The caller cancelled the flow.
    Cancelled,
    /// The ID token is malformed.
    InvalidToken,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "request failed: {e}"),
            Self::Server(e) => write!(f, "server error: {e}"),
            Self::Denied => f.write_str("access denied"),
            Self::Expired => f.write_str("the login expired, please try again"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::InvalidToken => f.write_str("received an invalid ID token"),
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
    // Google reports e.g. `authorization_pending` as HTTP 400 with a JSON body.
    Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

/// What the user has to do to complete the login.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    #[serde(alias = "verification_uri")]
    pub verification_url: String,
    /// URL with the user code already filled in, if the server provides one.
    #[serde(alias = "verification_url_complete")]
    pub verification_uri_complete: Option<String>,
    #[serde(default = "default_expires_in")]
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

impl DeviceAuthorization {
    /// URL that opens the verification page with the user code filled in.
    ///
    /// Google does not send a complete URL, but its verification page
    /// accepts the code as `user_code` query parameter.
    pub fn complete_url(&self) -> String {
        if let Some(url) = &self.verification_uri_complete {
            return url.clone();
        }
        let separator = if self.verification_url.contains('?') { '&' } else { '?' };
        let mut url = format!("{}{separator}user_code=", self.verification_url);
        for b in self.user_code.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                url.push(b as char);
            } else {
                url.push_str(&format!("%{b:02X}"));
            }
        }
        url
    }
}

fn default_expires_in() -> u64 {
    1800
}

fn default_interval() -> u64 {
    5
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
    error_description: Option<String>,
}

impl ErrorBody {
    fn message(self) -> String {
        match (self.error, self.error_description) {
            (Some(e), Some(d)) => format!("{e}: {d}"),
            (Some(e), None) => e,
            _ => "unknown error".into(),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
    #[serde(flatten)]
    error: ErrorBody,
}

/// OIDC device flow against Google.
pub struct DeviceFlowClient {
    client_id: String,
    client_secret: String,
    agent: Agent,
}

impl DeviceFlowClient {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            agent: agent(),
        }
    }

    /// Request a device and user code.
    pub fn start(&self) -> Result<DeviceAuthorization, Error> {
        let mut response = self
            .agent
            .post(DEVICE_CODE_URL)
            .send_form([
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("scope", SCOPE),
            ])?;
        if !response.status().is_success() {
            let body: ErrorBody = response.body_mut().read_json()?;
            return Err(Error::Server(body.message()));
        }
        Ok(response.body_mut().read_json()?)
    }

    /// Wait until the user has approved the login and return the ID token.
    ///
    /// Checked between requests, `cancel` aborts the wait with
    /// [`Error::Cancelled`].
    pub fn wait_for_id_token(
        &self,
        authorization: &DeviceAuthorization,
        cancel: &AtomicBool,
    ) -> Result<String, Error> {
        let deadline = Instant::now() + Duration::from_secs(authorization.expires_in);
        let mut interval = authorization.interval.max(1);
        loop {
            sleep(Duration::from_secs(interval), cancel)?;
            if Instant::now() >= deadline {
                return Err(Error::Expired);
            }
            let mut response = self.agent.post(TOKEN_URL).send_form([
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("device_code", authorization.device_code.as_str()),
                ("grant_type", DEVICE_GRANT),
            ])?;
            let body: TokenResponse = response.body_mut().read_json()?;
            if let Some(token) = body.id_token {
                return Ok(token);
            }
            match body.error.error.as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval += 5,
                Some("access_denied") => return Err(Error::Denied),
                Some("expired_token") => return Err(Error::Expired),
                _ => return Err(Error::Server(body.error.message())),
            }
        }
    }
}

/// Sleep for `duration`, waking up early with an error when cancelled.
fn sleep(duration: Duration, cancel: &AtomicBool) -> Result<(), Error> {
    const STEP: Duration = Duration::from_millis(200);
    let end = Instant::now() + duration;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let now = Instant::now();
        if now >= end {
            return Ok(());
        }
        thread::sleep(STEP.min(end - now));
    }
}

/// The e-mail address in an ID token.
///
/// The signature is not checked: the token comes straight from Google's token
/// endpoint over TLS, as the OIDC specification allows.
pub fn email_from_id_token(id_token: &str) -> Result<String, Error> {
    #[derive(Deserialize)]
    struct Claims {
        email: Option<String>,
    }
    let payload = id_token.split('.').nth(1).ok_or(Error::InvalidToken)?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| Error::InvalidToken)?;
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| Error::InvalidToken)?;
    claims.email.ok_or(Error::InvalidToken)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(payload: &str) -> String {
        format!("h.{}.s", URL_SAFE_NO_PAD.encode(payload))
    }

    #[test]
    fn reads_email_claim() {
        let t = token(r#"{"sub":"1","email":"a@example.org"}"#);
        assert_eq!(email_from_id_token(&t).unwrap(), "a@example.org");
    }

    #[test]
    fn rejects_malformed_tokens() {
        assert!(email_from_id_token("garbage").is_err());
        assert!(email_from_id_token(&token("{}")).is_err());
        assert!(email_from_id_token("h.!!!.s").is_err());
    }

    #[test]
    fn parses_device_authorization_with_defaults() {
        let a: DeviceAuthorization = serde_json::from_str(
            r#"{"device_code":"d","user_code":"U-C","verification_url":"https://www.google.com/device"}"#,
        )
        .unwrap();
        assert_eq!((a.expires_in, a.interval), (1800, 5));
    }

    fn authorization(complete: Option<&str>) -> DeviceAuthorization {
        DeviceAuthorization {
            device_code: "d".into(),
            user_code: "ABC-DEF-GHIJ".into(),
            verification_url: "https://www.google.com/device".into(),
            verification_uri_complete: complete.map(Into::into),
            expires_in: 1800,
            interval: 5,
        }
    }

    #[test]
    fn builds_complete_url() {
        assert_eq!(
            authorization(None).complete_url(),
            "https://www.google.com/device?user_code=ABC-DEF-GHIJ"
        );
        assert_eq!(authorization(Some("https://x/y")).complete_url(), "https://x/y");
    }

    #[test]
    fn cancel_interrupts_waiting() {
        let cancel = AtomicBool::new(true);
        assert!(matches!(
            sleep(Duration::from_secs(60), &cancel),
            Err(Error::Cancelled)
        ));
    }
}
