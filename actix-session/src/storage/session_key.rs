use std::convert::TryFrom;

/// A session key, the string stored in a client-side cookie to associate a user
/// with its session state on the backend.
///
/// ## Validation
///
/// Session keys are stored as cookies, therefore they cannot be arbitrary long.
/// We require session keys to be smaller than 4064 bytes.
///
/// ```rust
/// use std::convert::TryInto;
/// use actix_session::storage::SessionKey;
///
/// let key: String = std::iter::repeat('a').take(4065).collect();
/// let session_key: Result<SessionKey, _> = key.try_into();
/// assert!(session_key.is_err());
/// ```
#[derive(PartialEq, Eq)]
pub struct SessionKey(String);

impl TryFrom<String> for SessionKey {
    type Error = InvalidSessionKeyError;

    fn try_from(v: String) -> Result<Self, Self::Error> {
        if v.len() > 4064 {
            return Err(anyhow::anyhow!(
                "The session key is bigger than 4064 bytes, the upper limit on cookie content."
            )
            .into());
        }
        Ok(SessionKey(v))
    }
}

impl AsRef<str> for SessionKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<SessionKey> for String {
    fn from(k: SessionKey) -> Self {
        k.0
    }
}

#[derive(Debug, derive_more::Display, derive_more::From)]
#[display(fmt = "The provided string is not a valid session key")]
pub struct InvalidSessionKeyError(anyhow::Error);

impl std::error::Error for InvalidSessionKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}
