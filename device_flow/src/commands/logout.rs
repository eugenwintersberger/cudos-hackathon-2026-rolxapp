use crate::error::Result;
use crate::session::Session;

/// Deletes the stored session. Succeeds if there is none.
pub fn logout() -> Result<()> {
    Session::delete()?;
    Ok(())
}
