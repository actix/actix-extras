use actix_session::{SessionGetError, SessionInsertError};
use actix_web::{cookie::time::error::ComponentRange, http::StatusCode, ResponseError};

/// This error can occur during login attempts.
#[derive(Debug)]
pub struct LoginError(SessionInsertError);

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LoginError {}

impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }
}

impl From<SessionInsertError> for LoginError {
    fn from(error: SessionInsertError) -> Self {
        Self(error)
    }
}

/// This error describes all of the potential failures which can happen
/// while retrieving an identity.
#[derive(Debug)]
#[non_exhaustive]
pub enum IdentityError {
    /// This occurs whenever no identity is found in a session.
    MissingIdentityError(String),
    /// This occurs whenever something goes wrong accessing a session store.
    SessionGetError(SessionGetError),
    /// This occurs whenever any kind of expiration of a session has taken place.
    SessionExpiryError(ComponentRange),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIdentityError(message) => write!(f, "{}", message),
            Self::SessionExpiryError(source) => write!(f, "{}", source),
            Self::SessionGetError(source) => write!(f, "{}", source),
        }
    }
}

impl std::error::Error for IdentityError {}

impl ResponseError for IdentityError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }
}

impl From<ComponentRange> for IdentityError {
    fn from(error: ComponentRange) -> Self {
        Self::SessionExpiryError(error)
    }
}

impl From<SessionGetError> for IdentityError {
    fn from(source: SessionGetError) -> Self {
        Self::SessionGetError(source)
    }
}
