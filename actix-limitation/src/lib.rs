#[macro_use]
extern crate log;

use redis::Client;
use std::time::Duration;

pub use crate::core::{builder::Builder, errors::Error, status::Status};
pub use crate::middleware::RateLimiter;

pub const DEFAULT_REQUEST_LIMIT: usize = 5000;
pub const DEFAULT_PERIOD_SECS: u64 = 3600;
pub const DEFAULT_COOKIE_NAME: &str = "sid";
pub const DEFAULT_SESSION_KEY: &str = "rate-api-id";

#[derive(Clone, Debug)]
pub struct Limiter {
    client: Client,
    limit: usize,
    period: Duration,
    cookie_name: String,
    session_key: String,
}

impl Limiter {
    pub fn build(redis_url: &str) -> Builder {
        Builder {
            redis_url,
            limit: DEFAULT_REQUEST_LIMIT,
            period: Duration::from_secs(DEFAULT_PERIOD_SECS),
            cookie_name: DEFAULT_COOKIE_NAME.to_string(),
            session_key: DEFAULT_SESSION_KEY.to_string(),
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

        let mut connection = self.client.get_async_connection().await?;
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

mod core;
mod middleware;
#[cfg(test)]
mod test;
