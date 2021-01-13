//! Redis integration for Actix framework.
//!
//! ## Documentation
//! * [API Documentation (Development)](http://actix.github.io/actix-redis/actix_redis/)
//! * [API Documentation (Releases)](https://docs.rs/actix-redis/)
//! * [Chat on gitter](https://gitter.im/actix/actix)
//! * Cargo package: [actix-redis](https://crates.io/crates/actix-redis)
//! * Minimum supported Rust version: 1.40.0 or later

#![deny(rust_2018_idioms)]

mod cmd;
mod error;
mod redis;

pub use cmd::Command;
pub use error::Error;
pub use redis::RedisActor;

pub use redis_async::error::Error as RespError;
pub use redis_async::resp::RespValue;

#[cfg(feature = "web")]
mod session;
#[cfg(feature = "web")]
pub use actix_web::cookie::SameSite;
#[cfg(feature = "web")]
pub use session::RedisSession;
#[cfg(feature = "web")]
impl actix_web::ResponseError for Error {}
