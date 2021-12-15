use super::SessionKey;
use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::utils::generate_session_key;
use crate::storage::SessionStore;
use actix::Addr;
use actix_redis::{resp_array, RespValue};
use actix_redis::{Command, RedisActor};
use time::{self, Duration};

/// Use Redis as session storage backend.
///
/// ```no_run
/// use actix_web::{web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{SessionMiddleware, storage::RedisActorSessionStore};
/// use actix_web::cookie::Key;
///
/// // The secret key would usually be read from a configuration file/environment variables.
/// fn get_secret_key() -> Key {
///     # todo!()
///     // [...]
/// }
///
/// #[actix_rt::main]
/// async fn main() -> std::io::Result<()> {
///     let secret_key = get_secret_key();
///     let redis_connection_string = "127.0.0.1:6379";
///     HttpServer::new(move ||
///             App::new()
///             .wrap(
///                 SessionMiddleware::new(
///                     RedisActorSessionStore::new(redis_connection_string),
///                     secret_key.clone()
///                 )
///             )
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// ## Implementation notes
///
/// `RedisActorSessionStore` leverages `actix-redis`'s `RedisActor` implementation - each thread worker gets its
/// own connection to Redis.
///
/// ### Limitations
///
/// `RedisActorSessionStore` does not currently support establishing authenticated connections to Redis. Use
/// [`RedisSessionStore`] if you need TLS support.
///
/// [`RedisSessionStore`]: crate::storage::RedisSessionStorage
pub struct RedisActorSessionStore {
    configuration: CacheConfiguration,
    addr: Addr<RedisActor>,
}

impl RedisActorSessionStore {
    /// A fluent API to configure [`RedisActorSessionStore`].
    /// It takes as input the only required input to create a new instance of [`RedisActorSessionStore`] - a
    /// connection string for Redis.
    pub fn builder<S: Into<String>>(connection_string: S) -> RedisActorSessionStoreBuilder {
        RedisActorSessionStoreBuilder {
            configuration: Default::default(),
            connection_string: connection_string.into(),
        }
    }

    /// Create a new instance of [`RedisActorSessionStore`] using the default configuration.
    /// It takes as input the only required input to create a new instance of [`RedisActorSessionStore`] - a
    /// connection string for Redis.
    pub fn new<S: Into<String>>(connection_string: S) -> RedisActorSessionStore {
        Self::builder(connection_string).build()
    }
}

struct CacheConfiguration {
    cache_keygen: Box<dyn Fn(&str) -> String>,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            cache_keygen: Box::new(|s| s.to_owned()),
        }
    }
}

/// A fluent builder to construct a [`RedisActorSessionStore`] instance with custom
/// configuration parameters.
#[must_use]
pub struct RedisActorSessionStoreBuilder {
    connection_string: String,
    configuration: CacheConfiguration,
}

impl RedisActorSessionStoreBuilder {
    /// Set a custom cache key generation strategy, expecting a session key as input.
    pub fn cache_keygen<F>(mut self, keygen: F) -> Self
    where
        F: Fn(&str) -> String + 'static,
    {
        self.configuration.cache_keygen = Box::new(keygen);
        self
    }

    /// Finalise the builder and return a [`RedisActorSessionStore`] instance.
    #[must_use]
    pub fn build(self) -> RedisActorSessionStore {
        RedisActorSessionStore {
            configuration: self.configuration,
            addr: RedisActor::start(self.connection_string),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl SessionStore for RedisActorSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());
        let val = self
            .addr
            .send(Command(resp_array!["GET", cache_key]))
            .await
            .map_err(Into::into)
            .map_err(LoadError::GenericError)?
            .map_err(Into::into)
            .map_err(LoadError::GenericError)?;

        match val {
            RespValue::Error(e) => Err(LoadError::GenericError(anyhow::anyhow!(e))),
            RespValue::SimpleString(s) => Ok(serde_json::from_str(&s)
                .map_err(Into::into)
                .map_err(LoadError::DeserializationError)?),
            RespValue::BulkString(s) => Ok(serde_json::from_slice(&s)
                .map_err(Into::into)
                .map_err(LoadError::DeserializationError)?),
            _ => Ok(None),
        }
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(SaveError::SerializationError)?;
        let session_key = generate_session_key();
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let cmd = Command(resp_array![
            "SET",
            cache_key,
            body,
            "NX",
            "EX",
            &format!("{}", ttl.whole_seconds())
        ]);

        let result = self
            .addr
            .send(cmd)
            .await
            .map_err(Into::into)
            .map_err(SaveError::GenericError)?
            .map_err(Into::into)
            .map_err(SaveError::GenericError)?;
        match result {
            RespValue::SimpleString(_) => Ok(session_key),
            RespValue::Nil => Err(SaveError::GenericError(anyhow::anyhow!(
                "Failed to save session state. A record with the same key already existed in Redis"
            ))),
            e => Err(SaveError::GenericError(anyhow::anyhow!(
                "Failed to save session state. {:?}",
                e
            ))),
        }
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::SerializationError)?;
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let cmd = Command(resp_array![
            "SET",
            cache_key,
            body,
            "XX",
            "EX",
            &format!("{}", ttl.whole_seconds())
        ]);

        self.addr
            .send(cmd)
            .await
            .map_err(Into::into)
            .map_err(UpdateError::GenericError)?
            .map_err(Into::into)
            .map_err(UpdateError::GenericError)?;
        Ok(session_key)
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let res = self
            .addr
            .send(Command(resp_array!["DEL", cache_key]))
            .await?;

        match res {
            // Redis returns the number of deleted records
            Ok(RespValue::Integer(x)) if x > 0 => Ok(()),
            v => Err(anyhow::anyhow!(
                "Failed to remove session from cache. {:?}",
                v
            )),
        }
    }
}

// GitHub Actions do not support service containers (i.e. Redis, in our case) on
// non-Linux runners, therefore this test will fail in CI due to connection issues on those platform
#[cfg(test)]
#[cfg(target_os = "linux")]
mod test {
    use crate::storage::utils::generate_session_key;
    use crate::storage::{RedisActorSessionStore, SessionStore};
    use crate::test_helpers::acceptance_test_suite;

    fn redis_actor_store() -> RedisActorSessionStore {
        RedisActorSessionStore::new("127.0.0.1:6379")
    }

    #[actix_rt::test]
    async fn test_session_workflow() {
        acceptance_test_suite(redis_actor_store, true).await;
    }

    #[actix_rt::test]
    async fn loading_a_missing_session_returns_none() {
        let store = redis_actor_store();
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }
}
