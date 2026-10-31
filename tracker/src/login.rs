//! Sign-in at Rolx with the `device_flow` library (Google OAuth device flow).

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use device_flow::{AppConfig, ClientCredentials, DeviceCodePrompt, Session, SessionStore};
use tokio::sync::Notify;

use crate::config::LoginConfig;

/// Keeps the session in memory only.
#[derive(Default)]
pub struct MemorySessionStore(Mutex<Option<Session>>);

impl SessionStore for MemorySessionStore {
    fn load(&self) -> device_flow::Result<Option<Session>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, session: &Session) -> device_flow::Result<()> {
        *self.0.lock().unwrap() = Some(session.clone());
        Ok(())
    }

    fn delete(&self) -> device_flow::Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

/// Lets the UI abort a login that is waiting for the user.
#[derive(Default)]
pub struct Cancel {
    cancelled: AtomicBool,
    notify: Notify,
}

impl Cancel {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.notify.notify_one();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    async fn cancelled(&self) {
        if !self.is_cancelled() {
            self.notify.notified().await;
        }
    }
}

/// The settings of the `device_flow` library for our configuration.
pub fn app_config(login: &LoginConfig<'_>) -> AppConfig {
    AppConfig {
        server_url: login.server_url.trim_end_matches('/').to_owned(),
        google_desktop_client: None,
        google_device_client: Some(ClientCredentials {
            id: login.client_id.to_owned(),
            secret: login.client_secret.to_owned(),
        }),
    }
}

/// URL that opens the verification page with the user code filled in.
///
/// Google does not send a complete URL, but its verification page accepts the
/// code as `user_code` query parameter.
pub fn complete_url(prompt: &DeviceCodePrompt) -> String {
    if let Some(url) = &prompt.verification_uri_complete {
        return url.clone();
    }
    let separator = if prompt.verification_uri.contains('?') { '&' } else { '?' };
    let mut url = format!("{}{separator}user_code=", prompt.verification_uri);
    for b in prompt.user_code.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Run the device flow for `username`.
///
/// `show_code` is called as soon as the user can act. The call blocks until the
/// login is finished or cancelled, so run it in a thread of its own.
pub fn sign_in(
    config: &AppConfig,
    store: &MemorySessionStore,
    username: &str,
    cancel: &Cancel,
    show_code: impl FnOnce(&DeviceCodePrompt),
) -> Result<Session, String> {
    const CANCELLED: &str = "cancelled";

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let pending = tokio::select! {
            result = device_flow::login_code(config) => result.map_err(|e| e.to_string())?,
            () = cancel.cancelled() => return Err(CANCELLED.to_owned()),
        };
        show_code(pending.prompt());

        // Dropping the pending login on cancellation stops it.
        let session = tokio::select! {
            result = pending.finish(store) => result.map_err(|e| e.to_string())?,
            () = cancel.cancelled() => return Err(CANCELLED.to_owned()),
        };

        // The device flow cannot be bound to an account in advance, so make sure
        // that the person who confirmed the login is the one who was asked for.
        if !session.user_email.eq_ignore_ascii_case(username) {
            let _ = store.delete();
            return Err(format!(
                "signed in as {}, but {username} was expected",
                session.user_email
            ));
        }
        Ok(session)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(complete: Option<&str>) -> DeviceCodePrompt {
        DeviceCodePrompt {
            verification_uri: "https://www.google.com/device".into(),
            verification_uri_complete: complete.map(Into::into),
            user_code: "ABC-DEF-GHIJ".into(),
            expires_in: std::time::Duration::from_secs(1800),
        }
    }

    #[test]
    fn builds_complete_url() {
        assert_eq!(
            complete_url(&prompt(None)),
            "https://www.google.com/device?user_code=ABC-DEF-GHIJ"
        );
        assert_eq!(complete_url(&prompt(Some("https://x/y"))), "https://x/y");
    }

    #[test]
    fn uses_the_device_client_credentials() {
        let config = app_config(&LoginConfig {
            server_url: "https://rolx.example/",
            client_id: "id",
            client_secret: "secret",
        });
        assert_eq!(config.server_url, "https://rolx.example");
        assert!(config.google_desktop_client.is_none());
        assert_eq!(config.google_device_client.unwrap().id, "id");
    }

    #[test]
    fn cancel_is_remembered() {
        let cancel = Cancel::default();
        cancel.cancel();
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        runtime.block_on(cancel.cancelled());
    }
}
