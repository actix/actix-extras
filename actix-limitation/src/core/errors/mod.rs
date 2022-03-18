use std::{error, fmt};

use crate::core::status::Status;

#[derive(Debug)]
pub enum Error {
    /// The Redis client failed to connect or run a query.
    Client(redis::RedisError),
    /// The limit is exceeded for a key.
    LimitExceeded(Status),
    /// A time conversion failed.
    Time(time::ComponentRangeError),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Client(ref err) => write!(f, "Client error ({})", err),
            Error::LimitExceeded(ref status) => write!(f, "Rate limit exceeded ({:?})", status),
            Error::Time(ref err) => write!(f, "Time conversion error ({})", err),
            Error::Other(err) => write!(f, "{}", err),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Client(ref err) => err.source(),
            Error::LimitExceeded(_) => None,
            Error::Time(ref err) => err.source(),
            Error::Other(_) => None,
        }
    }
}

impl From<redis::RedisError> for Error {
    fn from(err: redis::RedisError) -> Self {
        Error::Client(err)
    }
}

impl From<time::ComponentRangeError> for Error {
    fn from(err: time::ComponentRangeError) -> Self {
        Error::Time(err)
    }
}
