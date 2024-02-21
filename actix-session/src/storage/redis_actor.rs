use actix::Addr;
use actix_redis::{resp_array, Command, RedisActor, RespValue};
use actix_web::cookie::time::Duration;
use anyhow::Error;

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
/// use actix_session::{SessionMiddleware, storage::RedisActorSessionStore};
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
/// # Implementation notes
///
/// `RedisActorSessionStore` leverages `actix-redis`'s `RedisActor` implementation - each thread
/// worker gets its own connection to Redis.
///
/// ## Limitations
///
/// `RedisActorSessionStore` does not currently support establishing authenticated connections to
/// Redis. Use [`RedisSessionStore`] if you need TLS support.
///
/// [`RedisSessionStore`]: crate::storage::RedisSessionStore
pub struct RedisActorSessionStore {
    configuration: CacheConfiguration,
    addr: Addr<RedisActor>,
}

impl RedisActorSessionStore {
    /// A fluent API to configure [`RedisActorSessionStore`].
    ///
    /// It takes as input the only required input to create a new instance of
    /// [`RedisActorSessionStore`]—a connection string for Redis.
    pub fn builder<S: Into<String>>(connection_string: S) -> RedisActorSessionStoreBuilder {
        RedisActorSessionStoreBuilder {
            configuration: CacheConfiguration::default(),
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
            cache_keygen: Box::new(str::to_owned),
        }
    }
}

/// A fluent builder to construct a [`RedisActorSessionStore`] instance with custom configuration
/// parameters.
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

impl SessionStore for RedisActorSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());
        let val = self
            .addr
            .send(Command(resp_array!["GET", cache_key]))
            .await
            .map_err(Into::into)
            .map_err(LoadError::Other)?
            .map_err(Into::into)
            .map_err(LoadError::Other)?;

        match val {
            RespValue::Error(err) => Err(LoadError::Other(anyhow::anyhow!(err))),

            RespValue::SimpleString(s) => Ok(serde_json::from_str(&s)
                .map_err(Into::into)
                .map_err(LoadError::Deserialization)?),

            RespValue::BulkString(s) => Ok(serde_json::from_slice(&s)
                .map_err(Into::into)
                .map_err(LoadError::Deserialization)?),

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
            .map_err(SaveError::Serialization)?;
        let session_key = generate_session_key();
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let cmd = Command(resp_array![
            "SET",
            cache_key,
            body,
            "NX", // NX: only set the key if it does not already exist
            "EX", // EX: set expiry
            format!("{}", ttl.whole_seconds())
        ]);

        let result = self
            .addr
            .send(cmd)
            .await
            .map_err(Into::into)
            .map_err(SaveError::Other)?
            .map_err(Into::into)
            .map_err(SaveError::Other)?;

        match result {
            RespValue::SimpleString(_) => Ok(session_key),
            RespValue::Nil => Err(SaveError::Other(anyhow::anyhow!(
                "Failed to save session state. A record with the same key already existed in Redis"
            ))),
            err => Err(SaveError::Other(anyhow::anyhow!(
                "Failed to save session state. {:?}",
                err
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
            .map_err(UpdateError::Serialization)?;
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let cmd = Command(resp_array![
            "SET",
            cache_key,
            body,
            "XX", // XX: Only set the key if it already exist.
            "EX", // EX: set expiry
            format!("{}", ttl.whole_seconds())
        ]);

        let result = self
            .addr
            .send(cmd)
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        match result {
            RespValue::Nil => {
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
            RespValue::SimpleString(_) => Ok(session_key),
            val => Err(UpdateError::Other(anyhow::anyhow!(
                "Failed to update session state. {:?}",
                val
            ))),
        }
    }

    async fn update_ttl(&self, session_key: &SessionKey, ttl: &Duration) -> Result<(), Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let cmd = Command(resp_array![
            "EXPIRE",
            cache_key,
            ttl.whole_seconds().to_string()
        ]);

        match self.addr.send(cmd).await? {
            Ok(RespValue::Integer(_)) => Ok(()),
            val => Err(anyhow::anyhow!(
                "Failed to update the session state TTL: {:?}",
                val
            )),
        }
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let res = self
            .addr
            .send(Command(resp_array!["DEL", cache_key]))
            .await?;

        match res {
            // Redis returns the number of deleted records
            Ok(RespValue::Integer(_)) => Ok(()),
            val => Err(anyhow::anyhow!(
                "Failed to remove session from cache. {:?}",
                val
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::test_helpers::acceptance_test_suite;

    fn redis_actor_store() -> RedisActorSessionStore {
        RedisActorSessionStore::new("127.0.0.1:6379")
    }

    #[actix_web::test]
    async fn test_session_workflow() {
        acceptance_test_suite(redis_actor_store, true).await;
    }

    #[actix_web::test]
    async fn loading_a_missing_session_returns_none() {
        let store = redis_actor_store();
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn updating_of_an_expired_state_is_handled_gracefully() {
        let store = redis_actor_store();
        let session_key = generate_session_key();
        let initial_session_key = session_key.as_ref().to_owned();
        let updated_session_key = store
            .update(session_key, HashMap::new(), &Duration::seconds(1))
            .await
            .unwrap();
        assert_ne!(initial_session_key, updated_session_key.as_ref());
    }
}
