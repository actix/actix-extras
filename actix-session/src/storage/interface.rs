use std::collections::HashMap;

pub(crate) type SessionState = HashMap<String, String>;

/// The interface to retrieve and save the current session data from/to the
/// chosen storage backend.
///
/// `actix-session` provides two implementations of session storage:
///
/// - a cookie-based one, [`CookieSession`], using a cookie to store and
/// retrieve session data;
/// - a cache-based one, [`RedisActorSession`], which stores session data
/// in a Redis instance.
///
/// You can provide your own custom session store backend by implementing this trait.
///
/// [`CookieSession`]: crate::CookieSession
/// [`RedisActorSession`]: crate::RedisActorSession
#[async_trait::async_trait(?Send)]
pub trait SessionStore {
    /// Load the session state associated to a session key.
    async fn load(&self, session_key: &str) -> Result<Option<SessionState>, LoadError>;

    /// Persist the session state for a newly created session.
    /// It returns the corresponding session key.
    async fn save(&self, session_state: SessionState) -> Result<String, SaveError>;

    /// Update the session state associated to a pre-existing session key.
    // TODO: add error type
    async fn update(
        &self,
        session_key: String,
        session_state: SessionState,
    ) -> Result<String, UpdateError>;

    /// Delete a session from the store.
    // TODO: add error type
    async fn delete(&self, session_key: &str) -> Result<(), anyhow::Error>;
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::load`].
pub enum LoadError {
    #[error("Failed to deserialize session state")]
    DeserializationError(#[source] anyhow::Error),
    #[error("Something went wrong when retrieving the session state.")]
    GenericError(#[source] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::save`].
pub enum SaveError {
    #[error("Failed to serialize session state")]
    SerializationError(#[source] anyhow::Error),
    #[error("Something went wrong when persisting the session state.")]
    GenericError(#[source] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::update`].
pub enum UpdateError {
    #[error("Failed to serialize session state")]
    SerializationError(#[source] anyhow::Error),
    #[error("Something went wrong when updating the session state.")]
    GenericError(#[source] anyhow::Error),
}
