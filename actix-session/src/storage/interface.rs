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

// We cannot derive the `Error` implementation using `derive_more` for our custom errors:
// `derive_more`'s `#[error(source)]` attribute requires the source implement the `Error` trait,
// while it's actually enough for it to be able to produce a reference to a dyn Error.

/// Possible failures modes for [`SessionStore::load`].
#[derive(Debug, derive_more::Display)]
pub enum LoadError {
    /// Failed to deserialize session state
    #[display(fmt = "Failed to deserialize session state")]
    DeserializationError(anyhow::Error),
    /// Something went wrong when retrieving the session state.
    #[display(fmt = "Something went wrong when retrieving the session state.")]
    GenericError(anyhow::Error),
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DeserializationError(e) => Some(e.as_ref()),
            Self::GenericError(e) => Some(e.as_ref())
        }
    }
}

#[derive(Debug, derive_more::Display)]
/// Possible failures modes for [`SessionStore::save`].
pub enum SaveError {
    /// Failed to serialize session state.
    #[display(fmt = "Failed to serialize session state")]
    SerializationError(anyhow::Error),
    /// Something went wrong when persisting the session state.
    #[display(fmt = "Something went wrong when persisting the session state.")]
    GenericError(anyhow::Error),
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SerializationError(e) => Some(e.as_ref()),
            Self::GenericError(e) => Some(e.as_ref())
        }
    }
}

#[derive(Debug, derive_more::Display)]
/// Possible failures modes for [`SessionStore::update`].
pub enum UpdateError {
    /// Failed to serialize session state
    #[display(fmt = "Failed to serialize session state")]
    SerializationError(anyhow::Error),
    /// Something went wrong when updating the session state.
    #[display(fmt = "Something went wrong when updating the session state.")]
    GenericError(anyhow::Error),
}

impl std::error::Error for UpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SerializationError(e) => Some(e.as_ref()),
            Self::GenericError(e) => Some(e.as_ref())
        }
    }
}
