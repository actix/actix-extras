use actix_session::{SessionGetError, SessionInsertError};
use actix_web::cookie::time::error::ComponentRange;

/// Possible errors which can emerge from storing and retrieving identities.
#[derive(Debug)]
pub enum IdentityError {
    /// This happens whenever no identity is found in a session.
    NoIdentity(String),
    /// Something has happened when getting the session value from the session store.
    SessionGetError {
        /// The source error, [SessionGetError]
        source: SessionGetError,
        /// Any additional messaging for the end user.
        message: String,
    },
    /// Identity has failed to store identity info in the session store.
    SessionInsertError(SessionInsertError),
    /// This occurs whenever any kind of expiration of a session occurs.
    SessionExpired(ComponentRange),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoIdentity(content) => write!(f, "{}", content),
            Self::SessionExpired(interior) => write!(f, "{}", interior),
            Self::SessionInsertError(interior) => write!(f, "{}", interior),
            Self::SessionGetError { source, message } => {
                write!(
                    f,
                    "{} \
                    {}",
                    message, source
                )
            }
        }
    }
}

impl std::error::Error for IdentityError {}

impl From<ComponentRange> for IdentityError {
    fn from(error: ComponentRange) -> Self {
        Self::SessionExpired(error)
    }
}

impl From<SessionInsertError> for IdentityError {
    fn from(error: SessionInsertError) -> Self {
        Self::SessionInsertError(error)
    }
}

impl From<SessionGetError> for IdentityError {
    fn from(error: SessionGetError) -> Self {
        Self::SessionGetError {
            source: error,
            message: "Failed to deserialize the user identifier attached to the current session"
                .to_owned(),
        }
    }
}
