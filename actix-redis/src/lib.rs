//! Redis integration for `actix`.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible)]

use derive_more::{Display, Error, From};
pub use redis_async::{error::Error as RespError, resp::RespValue, resp_array};

mod redis;
pub use self::redis::{Command, RedisActor};

/// General purpose `actix-redis` error.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    #[display(fmt = "Redis error: {}", _0)]
    Redis(redis_async::error::Error),

    /// Receiving message during reconnecting.
    #[display(fmt = "Redis: Not connected")]
    NotConnected,

    /// Cancel all waiters when connection is dropped.
    #[display(fmt = "Redis: Disconnected")]
    Disconnected,
}

#[cfg(feature = "web")]
impl actix_web::ResponseError for Error {}
