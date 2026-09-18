//! Pluggable storage backends for session state.

#[cfg(feature = "cookie-session")]
mod cookie;
#[cfg(any(
    feature = "cookie-session",
    feature = "redis-session",
    feature = "sled-session",
    test
))]
mod format;
mod interface;
#[cfg(feature = "redis-session")]
mod redis_rs;
mod session_key;
#[cfg(feature = "sled-session")]
mod sled;
mod utils;

#[cfg(feature = "cookie-session")]
pub use self::cookie::CookieSessionStore;
#[cfg(feature = "redis-session")]
pub use self::redis_rs::{RedisSessionStore, RedisSessionStoreBuilder};
#[cfg(feature = "sled-session")]
pub use self::sled::SledSessionStore;
pub use self::{
    interface::{LoadError, SaveError, SessionStore, UpdateError},
    session_key::SessionKey,
    utils::generate_session_key,
};
