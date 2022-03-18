use std::time::Duration;

use redis::Client;

use crate::{core::errors::Error, Limiter};

pub struct Builder<'builder> {
    pub(crate) redis_url: &'builder str,
    pub(crate) limit: usize,
    pub(crate) period: Duration,
    pub(crate) cookie_name: String,
    pub(crate) session_key: String,
}

impl Builder<'_> {
    pub fn limit(&mut self, limit: usize) -> &mut Self {
        self.limit = limit;
        self
    }

    pub fn period(&mut self, period: Duration) -> &mut Self {
        self.period = period;
        self
    }

    pub fn cookie_name(&mut self, cookie_name: String) -> &mut Self {
        self.cookie_name = cookie_name;
        self
    }

    pub fn session_key(&mut self, session_key: String) -> &mut Self {
        self.session_key = session_key;
        self
    }

    /// Finializes and returns a `Limiter`.
    ///
    /// Note that this method will connect to the Redis server to test its connection which is a
    /// **synchronous** operation.
    pub fn finish(&self) -> Result<Limiter, Error> {
        Ok(Limiter {
            client: Client::open(self.redis_url)?,
            limit: self.limit,
            period: self.period,
            cookie_name: self.cookie_name.to_string(),
            session_key: self.session_key.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_builder() {
        let redis_url = "redis://127.0.0.1";
        let period = Duration::from_secs(10);
        let builder = Builder {
            redis_url,
            limit: 100,
            period,
            cookie_name: "session".to_string(),
            session_key: "rate-api".to_string(),
        };

        assert_eq!(builder.redis_url, redis_url);
        assert_eq!(builder.limit, 100);
        assert_eq!(builder.period, period);
        assert_eq!(builder.session_key, "rate-api");
        assert_eq!(builder.cookie_name, "session");
    }

    #[test]
    fn test_create_limiter() {
        let redis_url = "redis://127.0.0.1";
        let period = Duration::from_secs(20);
        let mut builder = Builder {
            redis_url,
            limit: 100,
            period: Duration::from_secs(10),
            session_key: "key".to_string(),
            cookie_name: "sid".to_string(),
        };

        let limiter = builder
            .limit(200)
            .period(period)
            .cookie_name("session".to_string())
            .session_key("rate-api".to_string())
            .finish()
            .unwrap();

        assert_eq!(limiter.limit, 200);
        assert_eq!(limiter.period, period);
        assert_eq!(limiter.session_key, "rate-api");
        assert_eq!(limiter.cookie_name, "session");
    }

    #[test]
    #[should_panic = "Redis URL did not parse"]
    fn test_create_limiter_error() {
        let redis_url = "127.0.0.1";
        let period = Duration::from_secs(20);
        let mut builder = Builder {
            redis_url,
            limit: 100,
            period: Duration::from_secs(10),
            session_key: "key".to_string(),
            cookie_name: "sid".to_string(),
        };

        builder.limit(200).period(period).finish().unwrap();
    }
}
