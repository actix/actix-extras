mod interface;
pub use interface::{LoadError, SaveError, SessionStore, UpdateError};

#[cfg(feature = "cookie-session")]
pub use cookie::CookieSessionStore;
#[cfg(feature = "cookie-session")]
mod cookie;

#[cfg(feature = "redis-actor-session")]
pub use redis_actor::RedisActorSessionStore;
#[cfg(feature = "redis-actor-session")]
mod redis_actor;
