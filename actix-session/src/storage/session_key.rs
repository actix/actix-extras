use std::convert::TryFrom;

use derive_more::{Display, From};
use secrecy::{ExposeSecret, Secret};

/// A session key, the string stored in a client-side cookie to associate a user with its session
/// state on the backend.
///
/// # Validation
/// Session keys are stored as cookies, therefore they cannot be arbitrary long. Session keys are
/// required to be smaller than 4064 bytes.
///
/// ```rust
/// # use std::convert::TryInto;
/// use actix_session::storage::SessionKey;
///
/// let key: String = std::iter::repeat('a').take(4065).collect();
/// let session_key: Result<SessionKey, _> = key.try_into();
/// assert!(session_key.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct SessionKey(secrecy::Secret<String>);

impl SessionKey {
    /// Convert the SessionKey into the inner Secret
    pub fn into_inner(self) -> secrecy::Secret<String> {
        self.0
    }
}

impl TryFrom<String> for SessionKey {
    type Error = InvalidSessionKeyError;

    fn try_from(val: String) -> Result<Self, Self::Error> {
        if val.len() > 4064 {
            return Err(anyhow::anyhow!(
                "The session key is bigger than 4064 bytes, the upper limit on cookie content."
            )
            .into());
        }
        Ok(SessionKey(Secret::new(val)))
    }
}

impl AsRef<secrecy::Secret<String>> for SessionKey {
    fn as_ref(&self) -> &secrecy::Secret<String> {
        &self.0
    }
}

impl From<SessionKey> for String {
    fn from(key: SessionKey) -> Self {
        key.0.expose_secret().into()
    }
}

#[derive(Debug, Display, From)]
#[display(fmt = "The provided string is not a valid session key")]
pub struct InvalidSessionKeyError(anyhow::Error);

impl std::error::Error for InvalidSessionKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}
