use crate::storage::SessionKey;
use rand::distributions::Alphanumeric;
use rand::rngs::OsRng;
use rand::Rng;
use std::convert::TryInto;

/// This session key generation routine follows [OWASP's recommendations](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy).
pub(crate) fn generate_session_key() -> SessionKey {
    let value = std::iter::repeat(())
        .map(|()| OsRng.sample(Alphanumeric))
        .take(64)
        .collect::<Vec<_>>();
    // These unwraps will never panic because pre-conditions are always verified
    // (i.e. length and character set)
    String::from_utf8(value).unwrap().try_into().unwrap()
}
