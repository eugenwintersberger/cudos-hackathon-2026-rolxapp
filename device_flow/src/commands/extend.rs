use crate::commands::login::http_client;
use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::rolx::sign_in;
use crate::session::{Session, SessionStore};

/// Extends the stored session at Rolx and stores the extended one.
///
/// Fails with [`Error::NoSession`] if there is none and with [`Error::SessionExpired`] if Rolx
/// rejected it: log in again.
pub async fn extend_session(cfg: &AppConfig, store: &impl SessionStore) -> Result<Session> {
    let current = store.load()?.ok_or(Error::NoSession)?;
    let http = http_client()?;
    let session = sign_in::extend(&http, &cfg.server_url, &current.bearer_token).await?;
    store.save(&session)?;
    Ok(session)
}
