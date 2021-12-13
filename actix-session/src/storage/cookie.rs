//! Cookie based sessions. See docs for [`CookieSession`].

use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::SessionStore;
use crate::SessionKey;
use std::convert::TryInto;

#[derive(Default)]
#[non_exhaustive]
pub struct CookieSessionStore;

#[async_trait::async_trait(?Send)]
impl SessionStore for CookieSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        serde_json::from_str(session_key.as_ref())
            .map(Option::Some)
            .map_err(anyhow::Error::new)
            .map_err(LoadError::DeserializationError)
    }

    async fn save(&self, session_state: SessionState) -> Result<SessionKey, SaveError> {
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
    ) -> Result<SessionKey, UpdateError> {
        self.save(session_state).await.map_err(|e| match e {
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
