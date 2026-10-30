use crate::error::Result;
use crate::session::SessionStore;

/// Deletes the stored session. Succeeds if there is none. Does not revoke it at Rolx.
pub fn logout(store: &impl SessionStore) -> Result<()> {
    store.delete()
}
