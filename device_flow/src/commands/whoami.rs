use crate::error::{Error, Result};
use crate::session::{Session, SessionStore};

/// Loads the stored session, or fails with [`Error::NoSession`]. The session may be expired,
/// see [`Session::is_expired`].
///
/// The session contains the bearer token: do not log or print it.
pub fn whoami(store: &impl SessionStore) -> Result<Session> {
    store.load()?.ok_or(Error::NoSession)
}
