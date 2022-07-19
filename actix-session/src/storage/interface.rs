use std::collections::HashMap;

use actix_web::cookie::time::Duration;
use derive_more::Display;

use super::SessionKey;

pub(crate) type SessionState = HashMap<String, String>;

/// The interface to retrieve and save the current session data from/to the chosen storage backend.
///
/// You can provide your own custom session store backend by implementing this trait.
///
/// [`async-trait`](https://docs.rs/async-trait) is used for this trait's definition. Therefore, it
/// is required for implementations, too. In particular, we use the send-optional variant:
/// `#[async_trait(?Send)]`.
#[async_trait::async_trait(?Send)]
pub trait SessionStore {
    /// Loads the session state associated to a session key.
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError>;

    /// Persist the session state for a newly created session.
    ///
    /// Returns the corresponding session key.
    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError>;

    /// Updates the session state associated to a pre-existing session key.
    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError>;

    /// Updates the TTL of the session state associated to a pre-existing session key.
    async fn update_ttl(
        &self,
        session_key: &SessionKey,
        ttl: &Duration,
    ) -> Result<(), anyhow::Error>;

    /// Deletes a session from the store.
    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error>;
}

// We cannot derive the `Error` implementation using `derive_more` for our custom errors:
// `derive_more`'s `#[error(source)]` attribute requires the source implement the `Error` trait,
// while it's actually enough for it to be able to produce a reference to a dyn Error.

/// Possible failures modes for [`SessionStore::load`].
#[derive(Debug, Display)]
pub enum LoadError {
    /// Failed to deserialize session state.
    #[display(fmt = "Failed to deserialize session state")]
    Deserialization(anyhow::Error),

    /// Something went wrong when retrieving the session state.
    #[display(fmt = "Something went wrong when retrieving the session state")]
    Other(anyhow::Error),
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Deserialization(err) => Some(err.as_ref()),
            Self::Other(err) => Some(err.as_ref()),
        }
    }
}

/// Possible failures modes for [`SessionStore::save`].
#[derive(Debug, Display)]
pub enum SaveError {
    /// Failed to serialize session state.
    #[display(fmt = "Failed to serialize session state")]
    Serialization(anyhow::Error),

    /// Something went wrong when persisting the session state.
    #[display(fmt = "Something went wrong when persisting the session state")]
    Other(anyhow::Error),
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialization(err) => Some(err.as_ref()),
            Self::Other(err) => Some(err.as_ref()),
        }
    }
}

#[derive(Debug, Display)]
/// Possible failures modes for [`SessionStore::update`].
pub enum UpdateError {
    /// Failed to serialize session state.
    #[display(fmt = "Failed to serialize session state")]
    Serialization(anyhow::Error),

    /// Something went wrong when updating the session state.
    #[display(fmt = "Something went wrong when updating the session state.")]
    Other(anyhow::Error),
}

impl std::error::Error for UpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialization(err) => Some(err.as_ref()),
            Self::Other(err) => Some(err.as_ref()),
        }
    }
}
