//! Pluggable storage backends for session state.
#[cfg(feature = "cookie-session")]
#[cfg_attr(docsrs, doc(cfg(feature = "cookie-session")))]
pub use cookie::CookieSessionStore;
#[cfg(feature = "redis-actor-session")]
#[cfg_attr(docsrs, doc(cfg(feature = "redis-actor-session")))]
pub use redis_actor::{RedisActorSessionStore, RedisActorSessionStoreBuilder};

mod session_key;
pub use session_key::SessionKey;
mod interface;
pub use interface::{LoadError, SaveError, SessionStore, UpdateError};

#[cfg(feature = "cookie-session")]
mod cookie;
#[cfg(feature = "redis-actor-session")]
mod redis_actor;
