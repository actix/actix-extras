use rand::distributions::{Alphanumeric, DistString};

use crate::storage::SessionKey;

/// Session key generation routine that follows [OWASP recommendations].
///
/// [OWASP recommendations]: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy
pub fn generate_session_key() -> SessionKey {
    let session_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
    // This unwrap should never panic because the String is guaranteed to be 64 alphanumeric characters
    session_key.try_into().unwrap()
}
