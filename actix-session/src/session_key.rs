use std::convert::TryFrom;

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

#[derive(thiserror::Error, Debug)]
#[error("The provided string is not a valid session key")]
pub struct InvalidSessionKeyError(#[from] anyhow::Error);
