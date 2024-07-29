use rand::distributions::{Alphanumeric, DistString as _};

use crate::storage::SessionKey;

/// Session key generation routine that follows [OWASP recommendations].
///
/// [OWASP recommendations]: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy
pub fn generate_session_key() -> SessionKey {
    Alphanumeric
        .sample_string(&mut rand::thread_rng(), 64)
        .try_into()
        .expect("generated string should be within size range for a session key")
}
