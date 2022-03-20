use derive_more::{Display, Error, From};

use crate::status::Status;

/// Failure modes of the rate limiter.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// Redis client failed to connect or run a query.
    #[display(fmt = "Redis client failed to connect or run a query")]
    Client(redis::RedisError),

    /// Limit is exceeded for a key.
    #[display(fmt = "Limit is exceeded for a key")]
    #[from(ignore)]
    LimitExceeded(#[error(not(source))] Status),

    /// Time conversion failed.
    #[display(fmt = "Time conversion failed")]
    Time(time::error::ComponentRange),

    /// Generic error.
    #[display(fmt = "Generic error")]
    #[from(ignore)]
    Other(#[error(not(source))] String),
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all! {
        Error:
        From<redis::RedisError>,
        From<time::error::ComponentRange>,
    }

    static_assertions::assert_not_impl_any! {
        Error:
        From<String>,
        From<Status>,
    }
}
