use std::sync::Arc;

use actix_web::cookie::time::Duration;
use anyhow::Error;
#[cfg(not(feature = "redis-session"))]
use deadpool_redis::{redis, Pool};
#[cfg(feature = "redis-session")]
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Cmd, FromRedisValue, RedisResult, Value};

use super::SessionKey;
use crate::storage::{
    interface::{LoadError, SaveError, SessionState, UpdateError},
    utils::generate_session_key,
    SessionStore,
};

/// Use Redis as session storage backend.
///
/// ```no_run
/// use actix_web::{web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{SessionMiddleware, storage::RedisSessionStore};
/// use actix_web::cookie::Key;
///
/// // The secret key would usually be read from a configuration file/environment variables.
/// fn get_secret_key() -> Key {
///     # todo!()
///     // [...]
/// }
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     let secret_key = get_secret_key();
///     let redis_connection_string = "redis://127.0.0.1:6379";
///     let store = RedisSessionStore::new(redis_connection_string).await.unwrap();
///
///     HttpServer::new(move ||
///             App::new()
///             .wrap(SessionMiddleware::new(
///                 store.clone(),
///                 secret_key.clone()
///             ))
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// # TLS support
/// Add the `redis-rs-tls-session` or `redis-rs-tls-session-rustls` feature flag to enable TLS support. You can then establish a TLS
/// connection to Redis using the `rediss://` URL scheme:
///
/// ```no_run
/// use actix_session::{storage::RedisSessionStore};
///
/// # actix_web::rt::System::new().block_on(async {
/// let redis_connection_string = "rediss://127.0.0.1:6379";
/// let store = RedisSessionStore::new(redis_connection_string).await.unwrap();
/// # })
/// ```
///
/// # Deadpool Redis
///
/// ```ignore
/// use actix_session::storage::RedisSessionStore;
/// use deadpool_redis::{Config, Runtime};
///
/// let redis_cfg = Config::from_url("redis://127.0.0.1:6379");
/// let redis_pool = redis_cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
/// let store = RedisSessionStore::new(redis_pool.clone());
/// ```
///
/// # Implementation notes
/// `RedisSessionStore` leverages [`redis-rs`] as Redis client.
///
/// [`redis-rs`]: https://github.com/mitsuhiko/redis-rs
#[derive(Clone)]
pub struct RedisSessionStore {
    configuration: CacheConfiguration,
    #[cfg(feature = "redis-session")]
    client: ConnectionManager,
    #[cfg(not(feature = "redis-session"))]
    pool: Pool,
}

#[derive(Clone)]
struct CacheConfiguration {
    cache_keygen: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            cache_keygen: Arc::new(str::to_owned),
        }
    }
}

impl RedisSessionStore {
    /// A fluent API to configure [`RedisSessionStore`].
    /// It takes as input the only required input to create a new instance of [`RedisSessionStore`] - a
    /// connection string for Redis.
    #[cfg(feature = "redis-session")]
    pub fn builder<S: Into<String>>(connection_string: S) -> RedisSessionStoreBuilder {
        RedisSessionStoreBuilder {
            configuration: CacheConfiguration::default(),
            connection_string: connection_string.into(),
        }
    }

    /// A fluent API to configure [`RedisSessionStore`].
    /// It takes as input the only required input to create a new instance of [`RedisSessionStore`] - a
    /// pool object for Redis.
    #[cfg(not(feature = "redis-session"))]
    pub fn builder<P: Into<Pool>>(pool: P) -> RedisSessionStoreBuilder {
        RedisSessionStoreBuilder {
            configuration: CacheConfiguration::default(),
            pool: pool.into(),
        }
    }

    /// Create a new instance of [`RedisSessionStore`] using the default configuration.
    /// It takes as input the only required input to create a new instance of [`RedisSessionStore`] - a
    /// connection string for Redis.
    #[cfg(feature = "redis-session")]
    pub async fn new<S: Into<String>>(connection_string: S) -> Result<RedisSessionStore, Error> {
        Self::builder(connection_string).build().await
    }

    /// Create a new instance of [`RedisSessionStore`] using the default configuration.
    /// It takes as input the only required input to create a new instance of [`RedisSessionStore`] - a
    /// pool object for Redis.
    #[cfg(not(feature = "redis-session"))]
    pub fn new<P: Into<Pool>>(pool: P) -> RedisSessionStore {
        Self::builder(pool).build()
    }
}

/// A fluent builder to construct a [`RedisSessionStore`] instance with custom configuration
/// parameters.
///
/// [`RedisSessionStore`]: RedisSessionStore
#[must_use]
pub struct RedisSessionStoreBuilder {
    configuration: CacheConfiguration,
    #[cfg(feature = "redis-session")]
    connection_string: String,
    #[cfg(not(feature = "redis-session"))]
    pool: Pool,
}

impl RedisSessionStoreBuilder {
    /// Set a custom cache key generation strategy, expecting a session key as input.
    pub fn cache_keygen<F>(mut self, keygen: F) -> Self
    where
        F: Fn(&str) -> String + 'static + Send + Sync,
    {
        self.configuration.cache_keygen = Arc::new(keygen);
        self
    }

    /// Finalise the builder and return a [`RedisSessionStore`] instance.
    ///
    /// [`RedisSessionStore`]: RedisSessionStore
    #[cfg(feature = "redis-session")]
    pub async fn build(self) -> Result<RedisSessionStore, Error> {
        let client = ConnectionManager::new(redis::Client::open(self.connection_string)?).await?;
        Ok(RedisSessionStore {
            configuration: self.configuration,
            client,
        })
    }

    /// Finalise the builder and return a [`RedisSessionStore`] instance.
    ///
    /// [`RedisSessionStore`]: RedisSessionStore
    #[cfg(not(feature = "redis-session"))]
    pub fn build(self) -> RedisSessionStore {
        RedisSessionStore {
            configuration: self.configuration,
            pool: self.pool,
        }
    }
}

impl SessionStore for RedisSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let value: Option<String> = self
            .execute_command(redis::cmd("GET").arg(&[&cache_key]))
            .await
            .map_err(Into::into)
            .map_err(LoadError::Other)?;

        match value {
            None => Ok(None),
            Some(value) => Ok(serde_json::from_str(&value)
                .map_err(Into::into)
                .map_err(LoadError::Deserialization)?),
        }
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(SaveError::Serialization)?;
        let session_key = generate_session_key();
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.execute_command::<()>(
            redis::cmd("SET")
                .arg(&[
                    &cache_key, // key
                    &body,      // value
                    "NX",       // only set the key if it does not already exist
                    "EX",       // set expiry / TTL
                ])
                .arg(
                    ttl.whole_seconds(), // EXpiry in seconds
                ),
        )
        .await
        .map_err(Into::into)
        .map_err(SaveError::Other)?;

        Ok(session_key)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::Serialization)?;

        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let v: Value = self
            .execute_command(redis::cmd("SET").arg(&[
                &cache_key,
                &body,
                "XX", // XX: Only set the key if it already exist.
                "EX", // EX: set expiry
                &format!("{}", ttl.whole_seconds()),
            ]))
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        match v {
            Value::Nil => {
                // The SET operation was not performed because the XX condition was not verified.
                // This can happen if the session state expired between the load operation and the
                // update operation. Unlucky, to say the least. We fall back to the `save` routine
                // to ensure that the new key is unique.
                self.save(session_state, ttl)
                    .await
                    .map_err(|err| match err {
                        SaveError::Serialization(err) => UpdateError::Serialization(err),
                        SaveError::Other(err) => UpdateError::Other(err),
                    })
            }
            Value::Int(_) | Value::Okay | Value::Status(_) => Ok(session_key),
            val => Err(UpdateError::Other(anyhow::anyhow!(
                "Failed to update session state. {:?}",
                val
            ))),
        }
    }

    async fn update_ttl(&self, session_key: &SessionKey, ttl: &Duration) -> Result<(), Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        #[cfg(feature = "redis-session")]
        self.client
            .clone()
            .expire::<_, ()>(&cache_key, ttl.whole_seconds())
            .await?;

        #[cfg(not(feature = "redis-session"))]
        self.pool
            .get()
            .await?
            .expire(&cache_key, ttl.whole_seconds())
            .await?;

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.execute_command::<()>(redis::cmd("DEL").arg(&[&cache_key]))
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(())
    }
}

impl RedisSessionStore {
    /// Execute Redis command and retry once in certain cases.
    ///
    /// `ConnectionManager` automatically reconnects when it encounters an error talking to Redis.
    /// The request that bumped into the error, though, fails.
    ///
    /// This is generally OK, but there is an unpleasant edge case: Redis client timeouts. The
    /// server is configured to drop connections who have been active longer than a pre-determined
    /// threshold. `redis-rs` does not proactively detect that the connection has been dropped - you
    /// only find out when you try to use it.
    ///
    /// This helper method catches this case (`.is_connection_dropped`) to execute a retry. The
    /// retry will be executed on a fresh connection, therefore it is likely to succeed (or fail for
    /// a different more meaningful reason).
    #[allow(clippy::needless_pass_by_ref_mut)]
    async fn execute_command<T: FromRedisValue>(&self, cmd: &mut Cmd) -> RedisResult<T> {
        let mut can_retry = true;

        #[cfg(feature = "redis-session")]
        let client = &mut self.client.clone();

        #[cfg(not(feature = "redis-session"))]
        let client = &mut self.pool.get().await.unwrap();

        loop {
            match cmd.query_async(client).await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if can_retry && err.is_connection_dropped() {
                        tracing::debug!(
                            "Connection dropped while trying to talk to Redis. Retrying."
                        );

                        // Retry at most once
                        can_retry = false;

                        continue;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use actix_web::cookie::time;
    #[cfg(not(feature = "redis-session"))]
    use deadpool_redis::{Config, Runtime};

    use super::*;
    use crate::test_helpers::acceptance_test_suite;

    async fn redis_store() -> RedisSessionStore {
        #[cfg(feature = "redis-session")]
        {
            RedisSessionStore::new("redis://127.0.0.1:6379")
                .await
                .unwrap()
        }

        #[cfg(not(feature = "redis-session"))]
        {
            let redis_pool = Config::from_url("redis://127.0.0.1:6379")
                .create_pool(Some(Runtime::Tokio1))
                .unwrap();
            RedisSessionStore::new(redis_pool.clone())
        }
    }

    #[actix_web::test]
    async fn test_session_workflow() {
        let redis_store = redis_store().await;
        acceptance_test_suite(move || redis_store.clone(), true).await;
    }

    #[actix_web::test]
    async fn loading_a_missing_session_returns_none() {
        let store = redis_store().await;
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn loading_an_invalid_session_state_returns_deserialization_error() {
        let store = redis_store().await;
        let session_key = generate_session_key();

        #[cfg(feature = "redis-session")]
        store
            .client
            .clone()
            .set::<_, _, ()>(session_key.as_ref(), "random-thing-which-is-not-json")
            .await
            .unwrap();

        #[cfg(not(feature = "redis-session"))]
        store
            .pool
            .get()
            .await
            .unwrap()
            .set::<_, _, ()>(session_key.as_ref(), "random-thing-which-is-not-json")
            .await
            .unwrap();

        assert!(matches!(
            store.load(&session_key).await.unwrap_err(),
            LoadError::Deserialization(_),
        ));
    }

    #[actix_web::test]
    async fn updating_of_an_expired_state_is_handled_gracefully() {
        let store = redis_store().await;
        let session_key = generate_session_key();
        let initial_session_key = session_key.as_ref().to_owned();
        let updated_session_key = store
            .update(session_key, HashMap::new(), &time::Duration::seconds(1))
            .await
            .unwrap();
        assert_ne!(initial_session_key, updated_session_key.as_ref());
    }
}
