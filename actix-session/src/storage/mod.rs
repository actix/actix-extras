//! Pluggable storage backends for session state.

mod interface;
mod session_key;

pub use self::{
    interface::{LoadError, SaveError, SessionStore, UpdateError},
    session_key::SessionKey,
};

#[cfg(feature = "cookie-session")]
mod cookie;

#[cfg(feature = "redis-session")]
mod redis_rs;

#[cfg(feature = "redis-session")]
mod utils;

#[cfg(feature = "cookie-session")]
pub use self::cookie::CookieSessionStore;
#[cfg(feature = "redis-session")]
pub use self::redis_rs::{RedisSessionStore, RedisSessionStoreBuilder};
#[cfg(feature = "redis-session")]
pub use self::utils::generate_session_key;
