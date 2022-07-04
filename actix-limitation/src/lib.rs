//! Rate limiter using a fixed window counter for arbitrary keys, backed by Redis for Actix Web.
//!
//! ```toml
//! [dependencies]
//! actix-web = "4"
//! actix-limitation = "0.1.4"
//! ```
//!
//! ```no_run
//! use std::time::Duration;
//! use actix_web::{get, web, App, HttpServer, Responder};
//! use actix_limitation::{Limiter, RateLimiter};
//!
//! #[get("/{id}/{name}")]
//! async fn index(info: web::Path<(u32, String)>) -> impl Responder {
//!     format!("Hello {}! id:{}", info.1, info.0)
//! }
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let limiter = web::Data::new(
//!         Limiter::builder("redis://127.0.0.1")
//!             .cookie_name("session-id".to_owned())
//!             .session_key("rate-api-id".to_owned())
//!             .limit(5000)
//!             .period(Duration::from_secs(3600)) // 60 minutes
//!             .build()
//!             .unwrap(),
//!     );
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .wrap(RateLimiter)
//!             .app_data(limiter.clone())
//!             .service(index)
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run()
//!     .await
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible, missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]

use std::{borrow::Cow, time::Duration};

use redis::Client;

mod builder;
mod errors;
mod middleware;
mod status;

pub use self::builder::Builder;
pub use self::errors::Error;
pub use self::middleware::RateLimiter;
pub use self::status::Status;

/// Default request limit.
pub const DEFAULT_REQUEST_LIMIT: usize = 5000;

/// Default period (in seconds).
pub const DEFAULT_PERIOD_SECS: u64 = 3600;

/// Default cookie name.
pub const DEFAULT_COOKIE_NAME: &str = "sid";

/// Default session key.
pub const DEFAULT_SESSION_KEY: &str = "rate-api-id";

/// Rate limiter.
#[derive(Debug, Clone)]
pub struct Limiter {
    client: Client,
    limit: usize,
    period: Duration,
    cookie_name: Cow<'static, str>,
    session_key: Cow<'static, str>,
}

impl Limiter {
    /// Construct rate limiter builder with defaults.
    ///
    /// See [`redis-rs` docs](https://docs.rs/redis/0.21/redis/#connection-parameters) on connection
    /// parameters for how to set the Redis URL.
    #[must_use]
    pub fn builder(redis_url: &str) -> Builder<'_> {
        Builder {
            redis_url,
            limit: DEFAULT_REQUEST_LIMIT,
            period: Duration::from_secs(DEFAULT_PERIOD_SECS),
            cookie_name: Cow::Borrowed(DEFAULT_COOKIE_NAME),
            session_key: Cow::Borrowed(DEFAULT_SESSION_KEY),
        }
    }

    /// Consumes one rate limit unit, returning the status.
    pub async fn count(&self, key: impl Into<String>) -> Result<Status, Error> {
        let (count, reset) = self.track(key).await?;
        let status = Status::new(count, self.limit, reset);

        if count > self.limit {
            Err(Error::LimitExceeded(status))
        } else {
            Ok(status)
        }
    }

    /// Tracks the given key in a period and returns the count and TTL for the key in seconds.
    async fn track(&self, key: impl Into<String>) -> Result<(usize, usize), Error> {
        let key = key.into();
        let expires = self.period.as_secs();

        let mut connection = self.client.get_tokio_connection().await?;

        // The seed of this approach is outlined Atul R in a blog post about rate limiting using
        // NodeJS and Redis. For more details, see https://blog.atulr.com/rate-limiter
        let mut pipe = redis::pipe();
        pipe.atomic()
            .cmd("SET") // Set key and value
            .arg(&key)
            .arg(0)
            .arg("EX") // Set the specified expire time, in seconds.
            .arg(expires)
            .arg("NX") // Only set the key if it does not already exist.
            .ignore() // --- ignore returned value of SET command ---
            .cmd("INCR") // Increment key
            .arg(&key)
            .cmd("TTL") // Return time-to-live of key
            .arg(&key);

        let (count, ttl) = pipe.query_async(&mut connection).await?;
        let reset = Status::epoch_utc_plus(Duration::from_secs(ttl))?;

        Ok((count, reset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_limiter() {
        let builder = Limiter::builder("redis://127.0.0.1:6379/1");
        let limiter = builder.build();
        assert!(limiter.is_ok());

        let limiter = limiter.unwrap();
        assert_eq!(limiter.limit, 5000);
        assert_eq!(limiter.period, Duration::from_secs(3600));
        assert_eq!(limiter.cookie_name, DEFAULT_COOKIE_NAME);
        assert_eq!(limiter.session_key, DEFAULT_SESSION_KEY);
    }
}
