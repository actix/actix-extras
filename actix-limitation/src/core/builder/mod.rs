use redis::Client;
use std::time::Duration;

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
mod test;
