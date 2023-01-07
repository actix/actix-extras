//! Failure modes of identity operations.

use actix_session::{SessionGetError, SessionInsertError};
use actix_web::{cookie::time::error::ComponentRange, http::StatusCode, ResponseError};
use derive_more::{Display, Error, From};

/// Error that can occur during login attempts.
#[derive(Debug, Display, Error, From)]
#[display(fmt = "{_0}")]
pub struct LoginError(SessionInsertError);

impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }
}

/// Error encountered when working with a session that has expired.
#[derive(Debug, Display, Error)]
#[display(fmt = "The given session has expired and is no longer valid")]
pub struct SessionExpiryError(#[error(not(source))] pub(crate) ComponentRange);

/// The identity information has been lost.
///
/// Seeing this error in user code indicates a bug in actix-identity.
#[derive(Debug, Display, Error)]
#[display(
    fmt = "The identity information in the current session has disappeared after having been \
           successfully validated. This is likely to be a bug."
)]
#[non_exhaustive]
pub struct LostIdentityError;

/// There is no identity information attached to the current session.
#[derive(Debug, Display, Error)]
#[display(fmt = "There is no identity information attached to the current session")]
#[non_exhaustive]
pub struct MissingIdentityError;

/// Errors that can occur while retrieving an identity.
#[derive(Debug, Display, Error, From)]
#[non_exhaustive]
pub enum GetIdentityError {
    /// The session has expired.
    #[display(fmt = "{_0}")]
    SessionExpiryError(SessionExpiryError),

    /// No identity is found in a session.
    #[display(fmt = "{_0}")]
    MissingIdentityError(MissingIdentityError),

    /// Failed to accessing the session store.
    #[display(fmt = "{_0}")]
    SessionGetError(SessionGetError),

    /// Identity info was lost after being validated.
    ///
    /// Seeing this error indicates a bug in actix-identity.
    #[display(fmt = "{_0}")]
    LostIdentityError(LostIdentityError),
}

impl ResponseError for GetIdentityError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::LostIdentityError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::UNAUTHORIZED,
        }
    }
}
