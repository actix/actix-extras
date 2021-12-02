use actix_web::dev::ResponseHead;
use actix_web::HttpRequest;
use std::collections::HashMap;

pub(crate) type SessionState = HashMap<String, String>;
pub(crate) type SessionId = String;

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
pub trait SessionStore: Send + Sync {
    type SessionMetadata;

    /// Extract the session state from an incoming request.
    fn load(
        &self,
        request: &HttpRequest,
    ) -> Result<Option<(Self::SessionMetadata, SessionState)>, LoadError>;

    /// Persist the session state.
    fn save(
        &self,
        response: &mut ResponseHead,
        session: (Option<Self::SessionMetadata>, SessionState),
    ) -> Result<(), ()>;
}

#[derive(thiserror::Error, Debug)]
/// Possible failures modes for [`SessionStore::load`].
pub enum LoadError {
    #[error("Failed to deserialize session state")]
    DeserializationError(#[source] anyhow::Error),
    #[error(
        "The session failed a cryptographic integrity check (e.g. HMAC/signature verification)"
    )]
    IntegrityCheckFailed(#[source] anyhow::Error),
    #[error("Something went wrong when retrieving the session state.")]
    GenericError(#[source] anyhow::Error),
}
