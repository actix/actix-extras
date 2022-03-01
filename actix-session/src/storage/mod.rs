//! Pluggable storage backends for session state.

mod interface;
mod session_key;

pub use self::interface::{LoadError, SaveError, SessionStore, UpdateError};
pub use self::session_key::SessionKey;

#[cfg(feature = "cookie-session")]
mod cookie;

#[cfg(feature = "redis-actor-session")]
mod redis_actor;

#[cfg(feature = "redis-rs-session")]
mod redis_rs;

#[cfg(any(feature = "redis-actor-session", feature = "redis-rs-session"))]
mod utils;

#[cfg(feature = "cookie-session")]
pub use cookie::CookieSessionStore;

#[cfg(feature = "redis-actor-session")]
pub use redis_actor::{RedisActorSessionStore, RedisActorSessionStoreBuilder};

#[cfg(feature = "redis-rs-session")]
pub use redis_rs::{RedisSessionStore, RedisSessionStoreBuilder};
