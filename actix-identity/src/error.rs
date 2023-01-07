//! Failure modes of identity operations.

use std::fmt;

use actix_session::{SessionGetError, SessionInsertError};
use actix_web::{cookie::time::error::ComponentRange, http::StatusCode, ResponseError};

/// Error that can occur during login attempts.
#[derive(Debug)]
pub struct LoginError(SessionInsertError);

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// Error encountered when working with a session that has expired.
#[derive(Debug)]
pub struct SessionExpiryError(ComponentRange);

impl fmt::Display for SessionExpiryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The given session has expired and is no longer valid")
    }
}

/// The identity information has been lost.
///
/// Seeing this error in user code indicates a bug in actix-identity.
#[derive(Debug)]
#[non_exhaustive]
pub struct LostIdentityError;

impl fmt::Display for LostIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "The identity information in the current session has disappeared \
            after having been successfully validated. This is likely to be a bug.",
        )
    }
}

impl std::error::Error for LostIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}

/// There is no identity information attached to the current session.
#[derive(Debug)]
#[non_exhaustive]
pub struct MissingIdentityError;

impl fmt::Display for MissingIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("There is no identity information attached to the current session.")
    }
}

impl std::error::Error for MissingIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}

/// Errors that can occur while retrieving an identity.
#[derive(Debug)]
#[non_exhaustive]
pub enum GetIdentityError {
    /// The session has expired.
    SessionExpiryError(SessionExpiryError),

    /// No identity is found in a session.
    MissingIdentityError(MissingIdentityError),

    /// Failed to accessing the session store.
    SessionGetError(SessionGetError),

    /// Identity info was lost after being validated.
    ///
    /// Seeing this error indicates a bug in actix-identity.
    LostIdentityError(LostIdentityError),
}

impl fmt::Display for GetIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionExpiryError(err) => write!(f, "{err}"),
            Self::MissingIdentityError(err) => write!(f, "{err}"),
            Self::SessionGetError(err) => write!(f, "{err}"),
            Self::LostIdentityError(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for GetIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SessionExpiryError(err) => Some(err),
            Self::MissingIdentityError(err) => Some(err),
            Self::SessionGetError(err) => Some(err),
            Self::LostIdentityError(err) => Some(err),
        }
    }
}

impl ResponseError for GetIdentityError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }
}

impl From<LostIdentityError> for GetIdentityError {
    fn from(error: LostIdentityError) -> Self {
        Self::LostIdentityError(error)
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
