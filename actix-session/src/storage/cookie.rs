use super::SessionKey;
use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::SessionStore;
use std::convert::TryInto;
use time::Duration;

#[derive(Default)]
#[non_exhaustive]
/// Use the session key, stored in the session cookie, as storage backend for the session state.
///
/// ```no_run
/// use actix_web::{web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{SessionMiddleware, storage::CookieSessionStore};
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
/// ## Limitations
///
/// Cookies are subject to size limits - we require session keys to be shorter than 4096 bytes. This translates
/// into a limit on the maximum size of the session state when using cookies as storage backend.
///
/// The session cookie can always be inspected by end users via the developer tools exposed by their browsers.
/// We strongly recommend setting the content security policy to [`CookieContentSecurity::Private`] when using
/// cookies as storage backend.
///
/// There is no way to invalidate a session before its natural expiry when using cookies as storage backend.
///
/// [`CookieContentSecurity::Private`]: crate::CookieContentSecurity::Private
pub struct CookieSessionStore;

#[async_trait::async_trait(?Send)]
impl SessionStore for CookieSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        serde_json::from_str(session_key.as_ref())
            .map(Option::Some)
            .map_err(anyhow::Error::new)
            .map_err(LoadError::DeserializationError)
    }

    async fn save(
        &self,
        session_state: SessionState,
        _ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let session_key = serde_json::to_string(&session_state)
            .map_err(anyhow::Error::new)
            .map_err(SaveError::SerializationError)?;
        Ok(session_key
            .try_into()
            .map_err(Into::into)
            .map_err(SaveError::GenericError)?)
    }

    async fn update(
        &self,
        _session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        self.save(session_state, ttl).await.map_err(|e| match e {
            SaveError::SerializationError(e) => UpdateError::SerializationError(e),
            SaveError::GenericError(e) => UpdateError::GenericError(e),
        })
    }

    async fn delete(&self, _session_key: &SessionKey) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CookieSessionStore;
    use crate::test_helpers::acceptance_test_suite;

    #[actix_rt::test]
    async fn test_session_workflow() {
        acceptance_test_suite(|| CookieSessionStore::default(), false).await;
    }
}
