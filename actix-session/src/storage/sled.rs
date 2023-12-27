use std::{path::Path, sync::Arc};

use actix_web::cookie::time::Duration;
use async_trait::async_trait;

use super::{
    interface::SessionState, utils::generate_session_key, LoadError, SaveError, SessionKey,
    SessionStore, UpdateError,
};

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

/// TODO
#[cfg_attr(docsrs, doc(cfg(feature = "sled-session")))]
#[derive(Clone)]
pub struct SledSessionStore {
    configuration: CacheConfiguration,
    db: sled::Db,
}

impl SledSessionStore {
    /// TODO
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        Ok(Self {
            configuration: CacheConfiguration::default(),
            db: sled::open(db_path)?,
        })
    }
}

#[async_trait(?Send)]
impl SessionStore for SledSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let value = self
            .db
            .get(cache_key)
            .map_err(Into::into)
            .map_err(LoadError::Other)?;

        match value {
            None => Ok(None),
            Some(value) => Ok(serde_json::from_slice(&value)
                .map_err(Into::into)
                .map_err(LoadError::Deserialization)?),
        }
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let session_key = generate_session_key();

        self.update(session_key, session_state, ttl)
            .await
            .map_err(|err| match err {
                UpdateError::Serialization(err) => SaveError::Serialization(err),
                UpdateError::Other(err) => SaveError::Other(err),
            })
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        _ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let body = serde_json::to_vec(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::Serialization)?;

        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.db
            .insert(cache_key, body)
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(session_key)
    }

    async fn update_ttl(
        &self,
        session_key: &SessionKey,
        _ttl: &Duration,
    ) -> Result<(), anyhow::Error> {
        let _cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.db
            .drop_tree(&cache_key)
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use actix_web::cookie::time;

    use super::*;
    use crate::test_helpers::{acceptance_test_suite, function_name};

    fn sled_db(fn_name: &str) -> SledSessionStore {
        let db_name = fn_name.replace("::", ".").replace(".{{closure}}", "");

        SledSessionStore::new(
            [
                option_env!("CARGO_TARGET_DIR").unwrap_or("target"),
                "/tmp-actix-session-tests/",
                &db_name,
                "-",
                rand::random::<u16>().to_string().as_str(),
                ".db",
            ]
            .concat(),
        )
        .unwrap()
    }

    #[actix_web::test]
    async fn session_workflow() {
        let store = sled_db(function_name!());
        // TODO: use invalidation_supported = true
        acceptance_test_suite(move || store.clone(), false).await;
    }

    #[actix_web::test]
    async fn loading_a_missing_session_returns_none() {
        let store = sled_db(function_name!());
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn loading_an_invalid_session_state_returns_deserialization_error() {
        let store = sled_db(function_name!());
        let session_key = generate_session_key();

        store
            .db
            .insert(session_key.as_ref(), "random-thing-which-is-not-json")
            .unwrap();

        assert!(matches!(
            store.load(&session_key).await.unwrap_err(),
            LoadError::Deserialization(_),
        ));
    }

    // ignored until TTL handling is implemented
    #[ignore]
    #[actix_web::test]
    async fn updating_of_an_expired_state_is_handled_gracefully() {
        let store = sled_db(function_name!());
        let session_key = generate_session_key();
        let initial_session_key = session_key.as_ref().to_owned();

        let updated_session_key = store
            .update(session_key, HashMap::new(), &time::Duration::seconds(1))
            .await
            .unwrap();

        assert_ne!(initial_session_key, updated_session_key.as_ref());
    }
}
