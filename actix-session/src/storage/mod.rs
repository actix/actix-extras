//! Pluggable storage backends for session state.


mod interface;
#[cfg(any(feature = "redis-session", feature = "redis-dp-session"))]
mod redis_rs;
mod session_key;
#[cfg(any(feature = "redis-session", feature = "redis-dp-session"))]
mod utils;

#[cfg(feature = "cookie-session")]
pub use self::cookie::CookieSessionStore;
#[cfg(any(feature = "redis-session", feature = "redis-dp-session"))]
pub use self::redis_rs::{RedisSessionStore, RedisSessionStoreBuilder};
#[cfg(feature = "redis-session")]
pub use self::utils::generate_session_key;
pub use self::{
    interface::{LoadError, SaveError, SessionStore, UpdateError},
    session_key::SessionKey,
};
