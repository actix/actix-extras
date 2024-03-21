//! Rate limiter using a fixed window counter for arbitrary keys, backed by Redis for Actix Web.
//!
//! ```toml
//! [dependencies]
//! actix-web = "4"
#![doc = concat!("actix-limitation = \"", env!("CARGO_PKG_VERSION_MAJOR"), ".", env!("CARGO_PKG_VERSION_MINOR"),"\"")]
//! ```
//!
//! ```no_run
//! use std::{time::Duration};
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
//!     let limiter = web::Data::new(
//!         Limiter::builder("redis://127.0.0.1")
//!             .key_by(|req: &ServiceRequest| {
//!                 req.get_session()
//!                     .get("session-id")
//!                     .unwrap_or_else(|_| req.cookie("rate-api-id").map(|c| c.to_string()))
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

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible, missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use std::{borrow::Cow, fmt, sync::Arc, time::Duration};

use actix_web::dev::ServiceRequest;
use builder::RedisConnectionKind;
use redis::Client;

mod builder;
mod errors;
mod middleware;
mod status;

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
    client: Client,
    limit: usize,
    period: Duration,
    get_key_fn: GetArcBoxKeyFn,
}

impl Limiter {
    /// Construct rate limiter builder with defaults.
    ///
    /// See [`redis-rs` docs](https://docs.rs/redis/0.21/redis/#connection-parameters) on connection
    /// redis_url parameter is used for connecting to Redis.
    #[must_use]
    pub fn builder(redis_url: impl Into<String>) -> Builder {
        Builder {
            redis_connection: RedisConnectionKind::Url(redis_url.into()),
            limit: DEFAULT_REQUEST_LIMIT,
            period: Duration::from_secs(DEFAULT_PERIOD_SECS),
            get_key_fn: None,
            cookie_name: Cow::Borrowed(DEFAULT_COOKIE_NAME),
            #[cfg(feature = "session")]
            session_key: Cow::Borrowed(DEFAULT_SESSION_KEY),
        }
    }

    /// Construct rate limiter builder with defaults.
    ///
    /// parameters for how to set the Redis URL.
    #[must_use]
    pub fn builder_with_redis_client(redis_client: Client) -> Builder {
        Builder {
            redis_connection: RedisConnectionKind::Client(redis_client),
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
        let mut builder = Limiter::builder("redis://127.0.0.1:6379/1");
        let limiter = builder.build();
        assert!(limiter.is_ok());

        let limiter = limiter.unwrap();
        assert_eq!(limiter.limit, 5000);
        assert_eq!(limiter.period, Duration::from_secs(3600));
    }

    use std::{collections::HashMap, time::Duration};

    use actix_web::{
        dev::ServiceRequest, get, http::StatusCode, test as actix_test, web, Responder,
    };

    #[actix_web::test]
    async fn test_create_scoped_limiter() {
        #[get("/")]
        async fn index() -> impl Responder {
            "index"
        }

        let mut limiters = HashMap::new();
        let redis_client =
            Client::open("redis://127.0.0.1/2").expect("unable to create redis client");
        limiters.insert(
            "default",
            Limiter::builder_with_redis_client(redis_client.clone())
                .key_by(|_req: &ServiceRequest| Some("something_default".to_string()))
                .limit(5000)
                .period(Duration::from_secs(60))
                .build()
                .unwrap(),
        );
        limiters.insert(
            "scoped",
            Limiter::builder_with_redis_client(redis_client)
                .key_by(|_req: &ServiceRequest| Some("something_scoped".to_string()))
                .limit(1)
                .period(Duration::from_secs(60))
                .build()
                .unwrap(),
        );
        let limiters = web::Data::new(limiters);

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .wrap(RateLimiter::scoped("default"))
                .app_data(limiters.clone())
                .service(
                    web::scope("/scoped")
                        .wrap(RateLimiter::scoped("scoped"))
                        .service(index),
                )
                .service(index),
        )
        .await;

        for _ in 0..3 {
            let req = actix_test::TestRequest::get().uri("/").to_request();
            let resp = actix_test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK, "{:#?}", resp);
        }
        for request_count in 0..3 {
            let req = actix_test::TestRequest::get().uri("/scoped/").to_request();
            let resp = actix_test::call_service(&app, req).await;

            assert_eq!(
                resp.status(),
                if request_count > 0 {
                    StatusCode::TOO_MANY_REQUESTS
                } else {
                    StatusCode::OK
                },
                "{:#?}",
                resp
            );
        }
    }
}
