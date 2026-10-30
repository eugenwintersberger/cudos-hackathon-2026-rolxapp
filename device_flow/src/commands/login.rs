use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::oauth2::device_flow::{self, DeviceCodePrompt};
#[cfg(feature = "browser-login")]
use crate::oauth2::native_app_flow;
use crate::rolx::sign_in;
use crate::session::{Session, SessionStore};

// Uses Google OAuth, see:
// - https://developers.google.com/identity/oauth2/native-app
// - https://accounts.google.com/.well-known/oauth-authorization-server
#[cfg(feature = "browser-login")]
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_DEVICE_AUTH_URL: &str = "https://oauth2.googleapis.com/device/code";
const SCOPE: &str = "openid email profile";

pub(crate) fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .tls_version_min(reqwest::tls::Version::TLS_1_2)
        .build()?)
}

/// Exchanges the Google ID token for a Rolx session and stores it.
async fn complete(
    http: &reqwest::Client,
    app_cfg: &AppConfig,
    store: &impl SessionStore,
    id_token: Option<String>,
    token_kind: &'static str,
) -> Result<Session> {
    let id_token = id_token.ok_or(Error::MissingIdToken(token_kind))?;
    let session = sign_in::sign_in(http, &app_cfg.server_url, &id_token).await?;
    store.save(&session)?;

    tracing::info!(
        name = format!("{} {}", session.user_first_name, session.user_last_name),
        email = session.user_email,
        "Logged in as"
    );
    Ok(session)
}

/// A code login in progress: show [`prompt`](Self::prompt) to the user, then call
/// [`finish`](Self::finish). Dropping it cancels the login.
pub struct PendingCodeLogin {
    http: reqwest::Client,
    app_cfg: AppConfig,
    cfg: device_flow::Cfg,
    started: device_flow::Started,
}

impl PendingCodeLogin {
    /// Address to visit and code to enter. Never contains secrets.
    pub fn prompt(&self) -> &DeviceCodePrompt {
        &self.started.prompt
    }

    /// Waits until the user confirmed the login, then signs in to Rolx and stores the session.
    ///
    /// The returned session contains the bearer token: do not log or print it.
    #[tracing::instrument(name = "login_code.finish", skip_all)]
    pub async fn finish(self, store: &impl SessionStore) -> Result<Session> {
        let resp = device_flow::poll(&self.http, &self.cfg, &self.started).await?;
        complete(
            &self.http,
            &self.app_cfg,
            store,
            resp.id_token,
            "device flow token",
        )
        .await
    }
}

/// Starts a browserless login (OAuth 2.0 device flow). Needs `google_device_client` in the config.
#[tracing::instrument(name = "login_code.start", skip_all)]
pub async fn login_code(app_cfg: &AppConfig) -> Result<PendingCodeLogin> {
    tracing::info!(
        url = app_cfg.server_url,
        oauth_flow = "device",
        "Logging in to"
    );
    let credentials = app_cfg
        .google_device_client
        .as_ref()
        .ok_or(Error::MissingClientConfig("google_device_client"))?;
    let cfg = device_flow::Cfg {
        device_authorization_endpoint: GOOGLE_DEVICE_AUTH_URL.into(),
        token_endpoint: GOOGLE_TOKEN_URL.into(),
        client_id: credentials.id.clone(),
        client_secret: Some(credentials.secret.clone()),
    };
    let http = http_client()?;
    let started = device_flow::start(&http, &cfg, SCOPE).await?;
    Ok(PendingCodeLogin {
        http,
        app_cfg: app_cfg.clone(),
        cfg,
        started,
    })
}

/// A browser login in progress. Dropping it cancels the login and closes the redirect listener.
#[cfg(feature = "browser-login")]
pub struct PendingBrowserLogin {
    http: reqwest::Client,
    app_cfg: AppConfig,
    cfg: native_app_flow::Cfg,
    started: native_app_flow::Started,
}

#[cfg(feature = "browser-login")]
impl PendingBrowserLogin {
    /// `Some(url)` only if the browser could not be opened: the user must open the URL manually.
    pub fn manual_open_url(&self) -> Option<&str> {
        self.started.manual_open_url.as_deref()
    }

    /// Waits for the browser redirect, then signs in to Rolx and stores the session.
    ///
    /// The returned session contains the bearer token: do not log or print it.
    #[tracing::instrument(name = "login_browser.finish", skip_all)]
    pub async fn finish(self, store: &impl SessionStore) -> Result<Session> {
        let resp = native_app_flow::finish(&self.http, &self.cfg, self.started)
            .await
            .inspect_err(|e| tracing::error!(error = %e, "native app flow failed"))?;
        complete(
            &self.http,
            &self.app_cfg,
            store,
            resp.id_token,
            "native app flow token",
        )
        .await
    }
}

/// Starts a login in the browser (OAuth 2.0 native app flow with PKCE). Opens the browser.
/// Needs `google_desktop_client` in the config. The store is only read, to pre-select the
/// account of a previous login.
#[cfg(feature = "browser-login")]
#[tracing::instrument(name = "login_browser.start", skip_all)]
pub async fn login_browser(
    app_cfg: &AppConfig,
    store: &impl SessionStore,
) -> Result<PendingBrowserLogin> {
    tracing::info!(
        url = app_cfg.server_url,
        oauth_flow = "native-app",
        "Logging in to"
    );
    let credentials = app_cfg
        .google_desktop_client
        .as_ref()
        .ok_or(Error::MissingClientConfig("google_desktop_client"))?;
    let cfg = native_app_flow::Cfg {
        authorization_endpoint: GOOGLE_AUTH_URL.into(),
        token_endpoint: GOOGLE_TOKEN_URL.into(),
        client_id: credentials.id.clone(),
        client_secret: Some(credentials.secret.clone()),
        user_email: store.load().ok().flatten().map(|s| s.user_email),
    };
    let http = http_client()?;
    let started = native_app_flow::start(&mut rand::rng(), &cfg, SCOPE).await?;
    Ok(PendingBrowserLogin {
        http,
        app_cfg: app_cfg.clone(),
        cfg,
        started,
    })
}

#[cfg(test)]
#[path = "login.tests.rs"]
mod tests;
