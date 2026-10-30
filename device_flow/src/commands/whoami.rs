use crate::error::Result;
use crate::session::Session;

/// Loads the stored session.
///
/// The session contains the bearer token: do not log or print it.
pub fn whoami() -> Result<Session> {
    Ok(Session::load()?)
}
