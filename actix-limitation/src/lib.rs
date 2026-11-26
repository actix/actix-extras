//! Fixed-window rate limiting for Actix Web.
//!
//! [`Limiter::builder`] uses Redis to share counters across application instances. Enable
//! `redis-native-tls` or `redis-rustls` for TLS. The optional `memory-store` feature adds
//! `Limiter::memory_builder` for process-local counters.
//!
//! # Example
//!
//! Allow five requests per peer IP in a ten-second window:
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use actix_limitation::{Limiter, RateLimiter};
//! use actix_web::{web, App, HttpServer};
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let limiter = web::Data::new(
//!         Limiter::builder("redis://127.0.0.1")
//!             .key_by(|req| req.connection_info().peer_addr().map(str::to_owned))
//!             .limit(5)
//!             .period(Duration::from_secs(10))
//!             .build()
//!             .unwrap(),
//!     );
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(limiter.clone())
//!             .wrap(RateLimiter::default())
//!             .default_service(web::to(|| async { "Hello!" }))
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{borrow::Cow, fmt, sync::Arc, time::Duration};

use actix_web::dev::ServiceRequest;
use time::OffsetDateTime;

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

/// Helper trait to implement `Debug` for the key derivation function.
trait GetKeyFnT: Fn(&ServiceRequest) -> Option<String> {}

impl<T> GetKeyFnT for T where T: Fn(&ServiceRequest) -> Option<String> {}

type GetKeyFn = dyn GetKeyFnT + Send + Sync;

impl fmt::Debug for GetKeyFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GetKeyFn")
    }
}

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
    /// Constructs a rate limiter builder backed by Redis with default settings.
    ///
    /// See the [`redis` connection parameters docs](https://docs.rs/redis/1/redis/#connection-parameters)
    /// for supported URL formats.
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

    /// Constructs a rate limiter builder backed by an in-memory store.
    ///
    /// Counters are process-local and are not shared across instances. See [`MemoryStore`] for
    /// capacity, expiry, and deployment caveats.
    ///
    /// Pass [`MemoryStore::new()`] for defaults, or [`MemoryStore::builder()`] to configure the
    /// store.
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
        let reset = Status::epoch_utc_plus(OffsetDateTime::now_utc(), reset)?;
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
