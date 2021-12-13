use super::SessionKey;
use std::collections::HashMap;
use time::Duration;

pub(crate) type SessionState = HashMap<String, String>;

/// The interface to retrieve and save the current session data from/to the
/// chosen storage backend.
///
/// You can provide your own custom session store backend by implementing this trait.
#[async_trait::async_trait(?Send)]
pub trait SessionStore {
    /// Load the session state associated to a session key.
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError>;

    /// Persist the session state for a newly created session.
    /// It returns the corresponding session key.
    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError>;

    /// Update the session state associated to a pre-existing session key.
    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError>;

    /// Delete a session from the store.
    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error>;
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::load`].
pub enum LoadError {
    /// Failed to deserialize session state
    #[error("Failed to deserialize session state")]
    DeserializationError(#[source] anyhow::Error),
    /// Something went wrong when retrieving the session state.
    #[error("Something went wrong when retrieving the session state.")]
    GenericError(#[source] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::save`].
pub enum SaveError {
    /// Failed to serialize session state.
    #[error("Failed to serialize session state")]
    SerializationError(#[source] anyhow::Error),
    /// Something went wrong when persisting the session state.
    #[error("Something went wrong when persisting the session state.")]
    GenericError(#[source] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::update`].
pub enum UpdateError {
    /// Failed to serialize session state
    #[error("Failed to serialize session state")]
    SerializationError(#[source] anyhow::Error),
    /// Something went wrong when updating the session state.
    #[error("Something went wrong when updating the session state.")]
    GenericError(#[source] anyhow::Error),
}
