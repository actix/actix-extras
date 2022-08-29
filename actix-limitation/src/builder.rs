use std::{borrow::Cow, sync::Arc, time::Duration};

#[cfg(feature = "session")]
use actix_session::SessionExt;
use actix_web::dev::ServiceRequest;
use redis::Client;

use crate::{errors::Error, GetKeyFn, Limiter};

/// Rate limiter builder.
#[derive(Debug)]
pub struct Builder {
    pub(crate) redis_url: String,
    pub(crate) limit: usize,
    pub(crate) period: Duration,
    pub(crate) get_key_fn: Option<GetKeyFn>,
    pub(crate) cookie_name: Cow<'static, str>,
    #[cfg(feature = "session")]
    pub(crate) session_key: Cow<'static, str>,
}

impl Builder {
    /// Set upper limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set limit window/period.
    pub fn period(mut self, period: Duration) -> Self {
        self.period = period;
        self
    }

    /// Set the get_key method, This method should not be used in combination of cookie_name or session_key as they overwrite each other
    pub fn get_key(mut self, resolver: GetKeyFn) -> Self {
        self.get_key_fn = Some(resolver);
        self
    }

    /// Set name of cookie to be sent, This method should not be used in combination of get_key_fn as they overwrite each other
    #[deprecated = "prefer using get_key_fn"]
    pub fn cookie_name(mut self, cookie_name: impl Into<Cow<'static, str>>) -> Self {
        if self.get_key_fn.is_some() {
            panic!("This method should not be used in combination of get_key as they overwrite each other")
        }
        self.cookie_name = cookie_name.into();
        self
    }

    /// Set session key to be used in backend. This method should not be used in combination of get_key_fn as they overwrite each other
    #[deprecated = "prefer using get_key_fn"]
    #[cfg(feature = "session")]
    pub fn session_key(mut self, session_key: impl Into<Cow<'static, str>>) -> Self {
        if self.get_key_fn.is_some() {
            panic!("This method should not be used in combination of get_key as they overwrite each other")
        }
        self.session_key = session_key.into();
        self
    }

    /// Finalizes and returns a `Limiter`.
    ///
    /// Note that this method will connect to the Redis server to test its connection which is a
    /// **synchronous** operation.
    pub fn build(self) -> Result<Limiter, Error> {
        let get_key = if let Some(resolver) = self.get_key_fn {
            resolver
        } else {
            let cookie_name = self.cookie_name;
            #[cfg(feature = "session")]
            let session_key = self.session_key;
            Box::new(move |req: &ServiceRequest| {
                #[cfg(feature = "session")]
                let res = req
                    .get_session()
                    .get(&session_key)
                    .unwrap_or_else(|_| req.cookie(&cookie_name).map(|c| c.to_string()));
                #[cfg(not(feature = "session"))]
                let res = req.cookie(&cookie_name).map(|c| c.to_string());
                res
            })
        };
        Ok(Limiter {
            client: Client::open(self.redis_url.as_str())?,
            limit: self.limit,
            period: self.period,
            get_key_fn: Arc::new(get_key),
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
            redis_url: redis_url.to_owned(),
            limit: 100,
            period,
            get_key_fn: Some(Box::new(|_| None)),
            cookie_name: Cow::Owned("session".to_string()),
            #[cfg(feature = "session")]
            session_key: Cow::Owned("rate-api".to_string()),
        };

        assert_eq!(builder.redis_url, redis_url);
        assert_eq!(builder.limit, 100);
        assert_eq!(builder.period, period);
        #[cfg(feature = "session")]
        assert_eq!(builder.session_key, "rate-api");
        assert_eq!(builder.cookie_name, "session");
    }

    #[test]
    fn test_create_limiter() {
        let redis_url = "redis://127.0.0.1";
        let period = Duration::from_secs(20);
        let builder = Builder {
            redis_url: redis_url.to_owned(),
            limit: 100,
            period: Duration::from_secs(10),
            get_key_fn: Some(Box::new(|_| None)),
            cookie_name: Cow::Borrowed("sid"),
            #[cfg(feature = "session")]
            session_key: Cow::Borrowed("key"),
        };

        let limiter = builder.limit(200).period(period).build().unwrap();

        assert_eq!(limiter.limit, 200);
        assert_eq!(limiter.period, period);
    }

    #[test]
    #[should_panic = "Redis URL did not parse"]
    fn test_create_limiter_error() {
        let redis_url = "127.0.0.1";
        let period = Duration::from_secs(20);
        let builder = Builder {
            redis_url: redis_url.to_owned(),
            limit: 100,
            period: Duration::from_secs(10),
            get_key_fn: Some(Box::new(|_| None)),
            cookie_name: Cow::Borrowed("sid"),
            #[cfg(feature = "session")]
            session_key: Cow::Borrowed("key"),
        };

        builder.limit(200).period(period).build().unwrap();
    }
}
