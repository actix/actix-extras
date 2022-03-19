use std::{borrow::Cow, time::Duration};

use redis::Client;

use crate::{errors::Error, Limiter};

/// Rate limit builder.
#[derive(Debug)]
pub struct Builder<'a> {
    pub(crate) redis_url: &'a str,
    pub(crate) limit: usize,
    pub(crate) period: Duration,
    pub(crate) cookie_name: Cow<'static, str>,
    pub(crate) session_key: Cow<'static, str>,
}

impl Builder<'_> {
    /// Set upper limit.
    pub fn limit(&mut self, limit: usize) -> &mut Self {
        self.limit = limit;
        self
    }

    /// Set limit window/period.
    pub fn period(&mut self, period: Duration) -> &mut Self {
        self.period = period;
        self
    }

    /// Set name of cookie to be sent.
    pub fn cookie_name(&mut self, cookie_name: impl Into<Cow<'static, str>>) -> &mut Self {
        self.cookie_name = cookie_name.into();
        self
    }

    /// Set session key to be used in backend.
    pub fn session_key(&mut self, session_key: impl Into<Cow<'static, str>>) -> &mut Self {
        self.session_key = session_key.into();
        self
    }

    /// Finalizes and returns a `Limiter`.
    ///
    /// Note that this method will connect to the Redis server to test its connection which is a
    /// **synchronous** operation.
    pub fn build(&self) -> Result<Limiter, Error> {
        Ok(Limiter {
            client: Client::open(self.redis_url)?,
            limit: self.limit,
            period: self.period,
            cookie_name: self.cookie_name.clone(),
            session_key: self.session_key.clone(),
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
            cookie_name: Cow::Owned("session".to_string()),
            session_key: Cow::Owned("rate-api".to_string()),
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
            session_key: Cow::Borrowed("key"),
            cookie_name: Cow::Borrowed("sid"),
        };

        let limiter = builder
            .limit(200)
            .period(period)
            .cookie_name("session".to_string())
            .session_key("rate-api".to_string())
            .build()
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
            session_key: Cow::Borrowed("key"),
            cookie_name: Cow::Borrowed("sid"),
        };

        builder.limit(200).period(period).build().unwrap();
    }
}
