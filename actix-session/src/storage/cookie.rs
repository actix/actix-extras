use actix_web::cookie::time::Duration;
use anyhow::Error;

use super::SessionKey;
use crate::storage::{
    format::{deserialize_session_state, serialize_session_state},
    interface::{LoadError, SaveError, SessionState, UpdateError},
    SessionStore,
};

/// Use the session key, stored in the session cookie, as storage backend for the session state.
///
/// ```no_run
/// use actix_web::{cookie::Key, web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{SessionMiddleware, storage::CookieSessionStore};
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
///     HttpServer::new(move ||
///             App::new()
///             .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// # Limitations
/// Cookies are subject to size limits so we require session keys to be shorter than 4096 bytes.
/// This translates into a limit on the maximum size of the session state when using cookies as
/// storage backend.
///
/// The session cookie can always be inspected by end users via the developer tools exposed by their
/// browsers. We strongly recommend setting the policy to [`CookieContentSecurity::Private`] when
/// using cookies as storage backend.
///
/// There is no way to invalidate a session before its natural expiry when using cookies as the
/// storage backend.
///
/// [`CookieContentSecurity::Private`]: crate::config::CookieContentSecurity::Private
#[derive(Default)]
#[non_exhaustive]
pub struct CookieSessionStore;

impl SessionStore for CookieSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        deserialize_session_state(session_key.as_ref())
            .map(Some)
            .map_err(LoadError::Deserialization)
    }

    async fn save(
        &self,
        session_state: SessionState,
        _ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let session_key =
            serialize_session_state(&session_state).map_err(SaveError::Serialization)?;

        session_key
            .try_into()
            .map_err(Into::into)
            .map_err(SaveError::Other)
    }

    async fn update(
        &self,
        _session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        self.save(session_state, ttl)
            .await
            .map_err(|err| match err {
                SaveError::Serialization(err) => UpdateError::Serialization(err),
                SaveError::Other(err) => UpdateError::Other(err),
            })
    }

    async fn update_ttl(&self, _session_key: &SessionKey, _ttl: &Duration) -> Result<(), Error> {
        Ok(())
    }

    async fn delete(&self, _session_key: &SessionKey) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value};

    use super::*;
    use crate::{storage::utils::generate_session_key, test_helpers::acceptance_test_suite};

    #[actix_web::test]
    async fn test_session_workflow() {
        acceptance_test_suite(CookieSessionStore::default, false).await;
    }

    #[actix_web::test]
    async fn loading_a_random_session_key_returns_deserialization_error() {
        let store = CookieSessionStore::default();
        let session_key = generate_session_key();
        assert!(matches!(
            store.load(&session_key).await.unwrap_err(),
            LoadError::Deserialization(_),
        ));
    }

    #[actix_web::test]
    async fn saving_state_is_versioned_and_does_not_double_serialize_strings() {
        let store = CookieSessionStore::default();
        let mut state = Map::new();
        state.insert("k".into(), Value::from("value"));
        state.insert("n".into(), Value::from(1));

        let session_key = store.save(state, &Duration::seconds(60)).await.unwrap();

        // Stored cookie value should contain "value", not "\"value\"".
        let raw = session_key.as_ref();
        assert!(
            !raw.contains("\\\"value\\\""),
            "unexpected double-quoting: {raw}"
        );

        let decoded: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(decoded["v"], Value::from(1));
        assert_eq!(decoded["state"]["k"], Value::from("value"));
        assert_eq!(decoded["state"]["n"], Value::from(1));
    }

    #[actix_web::test]
    async fn legacy_state_format_is_migrated_on_load() {
        let store = CookieSessionStore::default();
        let legacy = serde_json::json!({
            "k": "\"value\"",
            "n": "1",
            "obj": "{\"a\": 1}"
        })
        .to_string();
        let legacy_key: SessionKey = legacy.try_into().unwrap();

        let state = store.load(&legacy_key).await.unwrap().unwrap();
        assert_eq!(state.get("k"), Some(&Value::from("value")));
        assert_eq!(state.get("n"), Some(&Value::from(1)));
        assert_eq!(state.get("obj"), Some(&serde_json::json!({"a": 1})));
    }
}
