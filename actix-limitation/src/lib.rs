//! Rate limiter using a fixed window counter for arbitrary keys, backed by Redis for Actix Web.
//!
//! ```toml
//! [dependencies]
//! actix-limitation = "0.1.4"
//! actix-web = "4"
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
//!         Limiter::build("redis://127.0.0.1")
//!             .cookie_name("session-id".to_owned())
//!             .session_key("rate-api-id".to_owned())
//!             .limit(5000)
//!             .period(Duration::from_secs(3600)) // 60 minutes
//!             .finish()
//!             .expect("Can't build actix-limiter"),
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

use std::borrow::Cow;
use std::time::Duration;

use redis::Client;

mod core;
mod middleware;

pub use self::core::{builder::Builder, errors::Error, status::Status};
pub use self::middleware::RateLimiter;

pub const DEFAULT_REQUEST_LIMIT: usize = 5000;
pub const DEFAULT_PERIOD_SECS: u64 = 3600;
pub const DEFAULT_COOKIE_NAME: &str = "sid";
pub const DEFAULT_SESSION_KEY: &str = "rate-api-id";

#[derive(Clone, Debug)]
pub struct Limiter {
    client: Client,
    limit: usize,
    period: Duration,
    cookie_name: Cow<'static, str>,
    session_key: Cow<'static, str>,
}

impl Limiter {
    pub fn build(redis_url: &str) -> Builder {
        Builder {
            redis_url,
            limit: DEFAULT_REQUEST_LIMIT,
            period: Duration::from_secs(DEFAULT_PERIOD_SECS),
            cookie_name: Cow::Borrowed(DEFAULT_COOKIE_NAME),
            session_key: Cow::Borrowed(DEFAULT_SESSION_KEY),
        }
    }

    pub async fn count<K: Into<String>>(&self, key: K) -> Result<Status, Error> {
        let (count, reset) = self.track(key).await?;
        let status = Status::build_status(count, self.limit, reset);

        if count > self.limit {
            Err(Error::LimitExceeded(status))
        } else {
            Ok(status)
        }
    }

    /// Tracks the given key in a period and returns the count and TTL for the key in seconds.
    async fn track<K: Into<String>>(&self, key: K) -> Result<(usize, usize), Error> {
        let key = key.into();
        let exipres = self.period.as_secs();

        let mut connection = self.client.get_tokio_connection().await?;

        // The seed of this approach is outlined Atul R in a blog post about rate limiting
        // using NodeJS and Redis. For more details, see
        // https://blog.atulr.com/rate-limiter/
        let mut pipe = redis::pipe();
        pipe.atomic()
            .cmd("SET")
            .arg(&key)
            .arg(0)
            .arg("EX")
            .arg(exipres)
            .arg("NX")
            .ignore()
            .cmd("INCR")
            .arg(&key)
            .cmd("TTL")
            .arg(&key);

        let (count, ttl): (usize, u64) = pipe.query_async(&mut connection).await?;
        let reset = Status::epoch_utc_plus(Duration::from_secs(ttl))?;
        Ok((count, reset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_limiter() {
        let builder = Limiter::build("redis://127.0.0.1:6379/1");
        let limiter = builder.finish();
        assert!(limiter.is_ok());

        let limiter = limiter.unwrap();
        assert_eq!(limiter.limit, 5000);
        assert_eq!(limiter.period, Duration::from_secs(3600));
        assert_eq!(limiter.cookie_name, DEFAULT_COOKIE_NAME);
        assert_eq!(limiter.session_key, DEFAULT_SESSION_KEY);
    }
}
