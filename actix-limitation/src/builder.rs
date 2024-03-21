use std::{borrow::Cow, sync::Arc, time::Duration};

#[cfg(feature = "session")]
use actix_session::SessionExt as _;
use actix_web::dev::ServiceRequest;
use redis::Client;

use crate::{errors::Error, GetArcBoxKeyFn, Limiter};

/// [RedisConnectionKind] is used to define which connection parameter for the Redis server will be passed
/// It can be an Url or a Client
/// This is done so we can use the same client for multiple Limiters
#[derive(Debug, Clone)]
pub enum RedisConnectionKind {
    Url(String),
    Client(Client),
}

/// Rate limiter builder.
#[derive(Debug)]
pub struct Builder {
    pub(crate) redis_connection: RedisConnectionKind,
    pub(crate) limit: usize,
    pub(crate) period: Duration,
    pub(crate) get_key_fn: Option<GetArcBoxKeyFn>,
    pub(crate) cookie_name: Cow<'static, str>,
    #[cfg(feature = "session")]
    pub(crate) session_key: Cow<'static, str>,
}

impl Builder {
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

    /// Sets rate limit key derivation function.
    ///
    /// Should not be used in combination with `cookie_name` or `session_key` as they conflict.
    pub fn key_by<F>(&mut self, resolver: F) -> &mut Self
    where
        F: Fn(&ServiceRequest) -> Option<String> + Send + Sync + 'static,
    {
        self.get_key_fn = Some(Arc::new(resolver));
        self
    }

    /// Sets name of cookie to be sent.
    ///
    /// This method should not be used in combination of `key_by` as they conflict.
    #[deprecated = "Prefer `key_by`."]
    pub fn cookie_name(&mut self, cookie_name: impl Into<Cow<'static, str>>) -> &mut Self {
        if self.get_key_fn.is_some() {
            panic!("This method should not be used in combination of get_key as they overwrite each other")
        }
        self.cookie_name = cookie_name.into();
        self
    }

    /// Sets session key to be used in backend.
    ///
    /// This method should not be used in combination of `key_by` as they conflict.
    #[deprecated = "Prefer `key_by`."]
    #[cfg(feature = "session")]
    pub fn session_key(&mut self, session_key: impl Into<Cow<'static, str>>) -> &mut Self {
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
    pub fn build(&mut self) -> Result<Limiter, Error> {
        let get_key = if let Some(resolver) = self.get_key_fn.clone() {
            resolver
        } else {
            let cookie_name = self.cookie_name.clone();

            #[cfg(feature = "session")]
            let session_key = self.session_key.clone();

            let closure: GetArcBoxKeyFn = Arc::new(Box::new(move |req: &ServiceRequest| {
                #[cfg(feature = "session")]
                let res = req
                    .get_session()
                    .get(&session_key)
                    .unwrap_or_else(|_| req.cookie(&cookie_name).map(|c| c.to_string()));

                #[cfg(not(feature = "session"))]
                let res = req.cookie(&cookie_name).map(|c| c.to_string());

                res
            }));
            closure
        };

        let client = match &self.redis_connection {
            RedisConnectionKind::Url(url) => Client::open(url.as_str())?,
            RedisConnectionKind::Client(client) => client.clone(),
        };
        Ok(Limiter {
            client,
            limit: self.limit,
            period: self.period,
            get_key_fn: get_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementing partial Eq to check if builder assigned the redis connection url correctly
    /// We can't / shouldn't compare Redis clients, thus the method panic if we try to compare them
    impl PartialEq for RedisConnectionKind {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Url(l_url), Self::Url(r_url)) => l_url == r_url,
                _ => {
                    panic!("RedisConnectionKind PartialEq is only implemented for Url")
                }
            }
        }
    }

    #[test]
    fn test_create_builder() {
        let redis_connection = RedisConnectionKind::Url("redis://127.0.0.1".to_string());
        let period = Duration::from_secs(10);
        let builder = Builder {
            redis_connection: redis_connection.clone(),
            limit: 100,
            period,
            get_key_fn: Some(Arc::new(|_| None)),
            cookie_name: Cow::Owned("session".to_string()),
            #[cfg(feature = "session")]
            session_key: Cow::Owned("rate-api".to_string()),
        };

        assert_eq!(builder.redis_connection, redis_connection);
        assert_eq!(builder.limit, 100);
        assert_eq!(builder.period, period);
        #[cfg(feature = "session")]
        assert_eq!(builder.session_key, "rate-api");
        assert_eq!(builder.cookie_name, "session");
    }

    #[test]
    fn test_create_limiter() {
        let redis_connection = RedisConnectionKind::Url("redis://127.0.0.1".to_string());
        let period = Duration::from_secs(20);
        let mut builder = Builder {
            redis_connection,
            limit: 100,
            period: Duration::from_secs(10),
            get_key_fn: Some(Arc::new(|_| None)),
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
        let redis_connection = RedisConnectionKind::Url("127.0.0.1".to_string());
        let period = Duration::from_secs(20);
        let mut builder = Builder {
            redis_connection,
            limit: 100,
            period: Duration::from_secs(10),
            get_key_fn: Some(Arc::new(|_| None)),
            cookie_name: Cow::Borrowed("sid"),
            #[cfg(feature = "session")]
            session_key: Cow::Borrowed("key"),
        };

        builder.limit(200).period(period).build().unwrap();
    }
}
