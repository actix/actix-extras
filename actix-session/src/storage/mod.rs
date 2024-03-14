//! Pluggable storage backends for session state.

mod interface;
mod session_key;

pub use self::{
    interface::{LoadError, SaveError, SessionStore, UpdateError},
    session_key::SessionKey,
};

#[cfg(feature = "cookie-session")]
mod cookie;

#[cfg(any(feature = "redis-rs-session", feature = "redis-dp-session"))]
mod redis_rs;

#[cfg(any(
    feature = "redis-rs-session",
    feature = "redis-dp-session"
))]
mod utils;

#[cfg(feature = "cookie-session")]
pub use cookie::CookieSessionStore;
#[cfg(any(feature = "redis-rs-session", feature = "redis-dp-session"))]
pub use redis_rs::{RedisSessionStore, RedisSessionStoreBuilder};
