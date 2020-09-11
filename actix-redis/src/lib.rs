//! Redis integration for Actix framework.
//!
//! ## Documentation
//! * [API Documentation (Development)](http://actix.github.io/actix-redis/actix_redis/)
//! * [API Documentation (Releases)](https://docs.rs/actix-redis/)
//! * [Chat on gitter](https://gitter.im/actix/actix)
//! * Cargo package: [actix-redis](https://crates.io/crates/actix-redis)
//! * Minimum supported Rust version: 1.40.0 or later

#![deny(rust_2018_idioms)]

mod redis;
pub use redis::{Command, RedisActor};

use derive_more::{Display, From};

#[cfg(feature = "web")]
mod session;
#[cfg(feature = "web")]
pub use actix_web::cookie::SameSite;
#[cfg(feature = "web")]
pub use session::RedisSession;

/// General purpose actix redis error
#[derive(Debug, Display, From)]
pub enum Error {
    #[display(fmt = "Redis error {}", _0)]
    Redis(redis_async::error::Error),
    /// Receiving message during reconnecting
    #[display(fmt = "Redis: Not connected")]
    NotConnected,
    /// Cancel all waters when connection get dropped
    #[display(fmt = "Redis: Disconnected")]
    Disconnected,
}

#[cfg(feature = "web")]
impl actix_web::ResponseError for Error {}

// re-export
pub use redis_async::error::Error as RespError;
pub use redis_async::resp::RespValue;
