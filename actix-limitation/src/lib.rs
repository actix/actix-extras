//! Rate limiter using a fixed window counter for arbitrary keys, backed by Redis or an in-memory
//! store, for Actix Web.
//!
//! ```toml
//! [dependencies]
//! actix-web = "4"
#![doc = concat!("actix-limitation = \"", env!("CARGO_PKG_VERSION_MAJOR"), ".", env!("CARGO_PKG_VERSION_MINOR"),"\"")]
//! ```
//!
//! # Choosing A Backend
//!
//! Counters are kept in a store, and a [`Limiter`] is bound to exactly one of them, chosen when it
//! is constructed. You can use:
//!
//! - a Redis-backed store, shared by every instance of your application, via [`Limiter::builder()`].
//!   The [`redis`] crate is a required dependency, so this backend needs no feature flag and is the
//!   default choice.
//!
//!   ```console
//!   cargo add actix-limitation
//!   ```
//!
//!   Add the `redis-native-tls` feature flag if you want to connect to Redis using a secure
//!   connection (via the `native-tls` crate):
//!
//!   ```console
//!   cargo add actix-limitation --features=redis-native-tls
//!   ```
//!
//!   If you, instead, prefer depending on `rustls`, use the `redis-rustls` feature flag:
//!
//!   ```console
//!   cargo add actix-limitation --features=redis-rustls
//!   ```
//!
//! - a process-local, in-memory store, [`MemoryStore`], via [`Limiter::memory_builder()`], using the
//!   `memory-store` feature flag. It needs no server at all, which makes it the zero-setup choice
//!   for local development, examples, and tests. Counters are not shared between instances, so read
//!   [`MemoryStore`] before reaching for it in production.
//!
//!   ```console
//!   cargo add actix-limitation --features=memory-store
//!   ```
//!
//! # Examples
//!
//! Rate limiting with Redis, keyed by session ID:
//!
//! ```no_run
//! use std::{sync::Arc, time::Duration};
//! use actix_web::{dev::ServiceRequest, get, web, App, HttpServer, Responder};
//! use actix_session::SessionExt as _;
//! use actix_limitation::{Limiter, RateLimiter};
//!
//! #[get("/{id}/{name}")]
//! async fn index(info: web::Path<(u32, String)>) -> impl Responder {
//!     format!("Hello {}! id:{}", info.1, info.0)
//! }
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     // Build the limiter once, outside the `HttpServer::new` closure, and clone the `web::Data`
//!     // handle into each worker; see the note on sharing below.
//!     let limiter = web::Data::new(
//!         Limiter::builder("redis://127.0.0.1")
//!             .key_by(|req: &ServiceRequest| {
//!                 req.get_session()
//!                     .get(&"session-id")
//!                     .unwrap_or_else(|_| req.cookie(&"rate-api-id").map(|c| c.to_string()))
//!             })
//!             .limit(5000)
//!             .period(Duration::from_secs(3600)) // 60 minutes
//!             .build()
//!             .unwrap(),
//!     );
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .wrap(RateLimiter::default())
//!             .app_data(limiter.clone())
//!             .service(index)
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```
//!
//! The same app rate limited by peer IP with no Redis server to run, using the `memory-store`
//! feature. Try it with `cargo run --example memory --features memory-store`.
//!
//! ```no_run
//! use std::time::Duration;
//! use actix_web::{dev::ServiceRequest, get, web, App, HttpServer, Responder};
//! use actix_limitation::{Limiter, MemoryStore, RateLimiter};
//!
//! #[get("/")]
//! async fn index() -> impl Responder {
//!     "Hello!"
//! }
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     // Build the limiter once, HERE, outside the `HttpServer::new` closure. The closure runs once
//!     // per worker thread, so constructing the limiter inside it would give every worker its own
//!     // set of counters and silently multiply the effective limit by the worker count. Cloning the
//!     // `web::Data` handle into each worker shares one store instead.
//!     let limiter = web::Data::new(
//!         Limiter::memory_builder(MemoryStore::new())
//!             .key_by(|req: &ServiceRequest| {
//!                 // `peer_addr` is the socket address, so unlike `realip_remote_addr` it cannot
//!                 // be spoofed with a forwarding header. Behind a trusted proxy, use the latter.
//!                 req.connection_info().peer_addr().map(str::to_owned)
//!             })
//!             .limit(5)
//!             .period(Duration::from_secs(10))
//!             .build()
//!             .unwrap(),
//!     );
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .wrap(RateLimiter::default())
//!             .app_data(limiter.clone())
//!             .service(index)
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```
//!
//! [`redis`]: https://docs.rs/redis
//! [`Limiter::builder()`]: Limiter::builder
//! [`Limiter::memory_builder()`]: Limiter::memory_builder
//! [`MemoryStore`]: crate::MemoryStore

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{borrow::Cow, fmt, sync::Arc, time::Duration};

use actix_web::dev::ServiceRequest;

mod builder;
mod errors;
mod middleware;
mod status;
mod store;

#[cfg(feature = "memory-store")]
#[cfg_attr(docsrs, doc(cfg(feature = "memory-store")))]
pub use self::store::{MemoryStore, MemoryStoreBuilder};
use self::{builder::BackendSpec, store::Backend};
pub use self::{builder::Builder, errors::Error, middleware::RateLimiter, status::Status};

/// Default request limit.
pub const DEFAULT_REQUEST_LIMIT: usize = 5000;

/// Default period (in seconds).
pub const DEFAULT_PERIOD_SECS: u64 = 3600;

/// Default cookie name.
pub const DEFAULT_COOKIE_NAME: &str = "sid";

/// Default session key.
#[cfg(feature = "session")]
pub const DEFAULT_SESSION_KEY: &str = "rate-api-id";

/// Helper trait to impl Debug on GetKeyFn type
trait GetKeyFnT: Fn(&ServiceRequest) -> Option<String> {}

impl<T> GetKeyFnT for T where T: Fn(&ServiceRequest) -> Option<String> {}

/// Get key function type with auto traits
type GetKeyFn = dyn GetKeyFnT + Send + Sync;

/// Get key resolver function type
impl fmt::Debug for GetKeyFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GetKeyFn")
    }
}

/// Wrapped Get key function Trait
type GetArcBoxKeyFn = Arc<GetKeyFn>;

/// Rate limiter.
#[derive(Debug, Clone)]
pub struct Limiter {
    backend: Backend,
    limit: usize,
    period: Duration,
    get_key_fn: GetArcBoxKeyFn,
}

impl Limiter {
    /// Construct rate limiter builder with defaults.
    ///
    /// See [`redis-rs` docs](https://docs.rs/redis/1.0.4/redis/#connection-parameters) on connection
    /// parameters for how to set the Redis URL.
    #[must_use]
    pub fn builder(redis_url: impl Into<String>) -> Builder {
        Builder {
            backend: BackendSpec::RedisUrl(redis_url.into()),
            limit: DEFAULT_REQUEST_LIMIT,
            period: Duration::from_secs(DEFAULT_PERIOD_SECS),
            get_key_fn: None,
            cookie_name: Cow::Borrowed(DEFAULT_COOKIE_NAME),
            #[cfg(feature = "session")]
            session_key: Cow::Borrowed(DEFAULT_SESSION_KEY),
        }
    }

    /// Construct rate limiter builder backed by the given in-memory store.
    ///
    /// Counters are process-local and are not shared between instances; see [`MemoryStore`] for
    /// what that means for your deployment.
    ///
    /// Pass [`MemoryStore::new()`] for defaults, or [`MemoryStore::builder()`] to configure it.
    #[cfg(feature = "memory-store")]
    #[cfg_attr(docsrs, doc(cfg(feature = "memory-store")))]
    #[must_use]
    pub fn memory_builder(store: MemoryStore) -> Builder {
        Builder {
            backend: BackendSpec::Memory(store),
            limit: DEFAULT_REQUEST_LIMIT,
            period: Duration::from_secs(DEFAULT_PERIOD_SECS),
            get_key_fn: None,
            cookie_name: Cow::Borrowed(DEFAULT_COOKIE_NAME),
            #[cfg(feature = "session")]
            session_key: Cow::Borrowed(DEFAULT_SESSION_KEY),
        }
    }

    /// Consumes one rate limit unit, returning the status.
    pub async fn count(&self, key: impl Into<String>) -> Result<Status, Error> {
        let (count, reset) = self.track(key).await?;
        let reset = Status::epoch_utc_plus(reset)?;
        let status = Status::new(count, self.limit, reset);

        if count > self.limit {
            Err(Error::LimitExceeded(status))
        } else {
            Ok(status)
        }
    }

    /// Tracks the given key in a period and returns the count and TTL for the key.
    async fn track(&self, key: impl Into<String>) -> Result<(usize, Duration), Error> {
        let key = key.into();

        match &self.backend {
            Backend::Redis(client) => store::redis::track(client, &key, self.period).await,

            #[cfg(feature = "memory-store")]
            Backend::Memory(store) => Ok(store.track(&key, self.period)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_limiter() {
        let mut builder = Limiter::builder("redis://127.0.0.1:6379/1");
        let limiter = builder.build();
        assert!(limiter.is_ok());

        let limiter = limiter.unwrap();
        assert_eq!(limiter.limit, 5000);
        assert_eq!(limiter.period, Duration::from_secs(3600));
    }
}
