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

impl std::error::Error for LoginError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

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

/// A wrapper for [ComponentRange] which adds expiration context.
#[derive(Debug)]
pub struct SessionExpiryError(ComponentRange);

impl std::fmt::Display for SessionExpiryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The given session has expired and is no longer valid")
    }
}

impl std::error::Error for SessionExpiryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// A marker left for any identity error nuance we may want to communicate.
#[derive(Debug)]
#[non_exhaustive]
pub struct MissingIdentityError;

impl std::fmt::Display for MissingIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for MissingIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}

/// This error describes all of the potential failures which can happen
/// while retrieving an identity.
#[derive(Debug)]
#[non_exhaustive]
pub enum GetIdentityError {
    /// This is an error which shouldn't occur, and indicates some kind of bug.
    LostIdentityError,
    /// This occurs whenever no identity is found in a session.
    MissingIdentityError(MissingIdentityError),

    /// This occurs whenever something goes wrong accessing a session store.
    SessionGetError(SessionGetError),
    /// This occurs whenever any kind of expiration of a session has taken place.
    SessionExpiryError(SessionExpiryError),
}

impl std::fmt::Display for GetIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LostIdentityError => write!(
                f,
                "Bug: the identity information attached to the current session has disappeared"
            ),
            Self::MissingIdentityError(_) => write!(
                f,
                "There is no identity information attached to the current session"
            ),
            Self::SessionExpiryError(source) => write!(f, "{}", source),
            Self::SessionGetError(source) => write!(f, "{}", source),
        }
    }
}

impl std::error::Error for GetIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LostIdentityError | Self::MissingIdentityError(_) => None,
            Self::SessionExpiryError(source) => Some(source),
            Self::SessionGetError(source) => Some(source),
        }
    }
}

impl ResponseError for GetIdentityError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }
}

impl From<MissingIdentityError> for GetIdentityError {
    fn from(error: MissingIdentityError) -> Self {
        Self::MissingIdentityError(error)
    }
}

impl From<ComponentRange> for GetIdentityError {
    fn from(error: ComponentRange) -> Self {
        Self::SessionExpiryError(SessionExpiryError(error))
    }
}

impl From<SessionGetError> for GetIdentityError {
    fn from(source: SessionGetError) -> Self {
        Self::SessionGetError(source)
    }
}
