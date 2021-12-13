use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::SessionStore;
use crate::SessionKey;
use actix::Addr;
use actix_redis::{resp_array, RespValue};
use actix_redis::{Command, RedisActor};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use std::convert::TryInto;
use time::{self, Duration};

/// Use redis as session storage.
///
/// You need to pass the address of the redis server to the constructor.
pub struct RedisActorSessionStore {
    configuration: CacheConfiguration,
    addr: Addr<RedisActor>,
}

impl RedisActorSessionStore {
    pub fn builder<S: Into<String>>(connection_string: S) -> RedisActorSessionStoreBuilder {
        RedisActorSessionStoreBuilder {
            configuration: Default::default(),
            connection_string: connection_string.into(),
        }
    }

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

pub struct RedisActorSessionStoreBuilder {
    connection_string: String,
    configuration: CacheConfiguration,
}

impl RedisActorSessionStoreBuilder {
    /// Set a custom cache key generation strategy, expecting a session key as input.
    pub fn cache_keygen(mut self, keygen: Box<dyn Fn(&str) -> String>) -> Self {
        self.configuration.cache_keygen = keygen;
        self
    }

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
            RespValue::Error(e) => {
                return Err(LoadError::GenericError(anyhow::anyhow!(e)));
            }
            RespValue::SimpleString(s) => {
                if let Ok(val) = serde_json::from_str(&s) {
                    return Ok(Some(val));
                }
            }
            RespValue::BulkString(s) => {
                if let Ok(val) = serde_json::from_slice(&s) {
                    return Ok(Some(val));
                }
            }
            _ => {}
        }

        Ok(None)
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
        let cache_key = (self.configuration.cache_keygen)(&session_key);

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
            RespValue::SimpleString(_) => Ok(session_key
                .try_into()
                .map_err(Into::into)
                .map_err(SaveError::GenericError)?),
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

/// This session key generation routine follows [OWASP's recommendations](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy).
fn generate_session_key() -> String {
    let value = std::iter::repeat(())
        .map(|()| OsRng.sample(Alphanumeric))
        .take(64)
        .collect::<Vec<_>>();
    String::from_utf8(value).unwrap()
}

#[cfg(test)]
mod test {
    use crate::test_helpers::acceptance_test_suite;
    use crate::RedisActorSessionStore;

    #[actix_rt::test]
    async fn test_session_workflow() {
        acceptance_test_suite(|| RedisActorSessionStore::new("127.0.0.1:6379"), true).await;
    }
}
