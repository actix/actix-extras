use actix_web::dev::ResponseHead;
use actix_web::HttpRequest;

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
    /// Extract flash messages from an incoming request.
    fn load(&self, request: &HttpRequest) -> Result<(), ()>;

    /// Attach flash messages to an outgoing response.
    fn store(&self, request: HttpRequest, response: &mut ResponseHead) -> Result<(), ()>;
}
