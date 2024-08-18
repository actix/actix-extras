//! Configuration options to tune the behaviour of [`SessionMiddleware`].

use actix_web::cookie::{time::Duration, Key, SameSite};
use derive_more::derive::From;

use crate::{storage::SessionStore, SessionMiddleware};

/// Determines what type of session cookie should be used and how its lifecycle should be managed.
///
/// Used by [`SessionMiddlewareBuilder::session_lifecycle`].
#[derive(Debug, Clone, From)]
#[non_exhaustive]
pub enum SessionLifecycle {
    /// The session cookie will expire when the current browser session ends.
    ///
    /// When does a browser session end? It depends on the browser! Chrome, for example, will often
    /// continue running in the background when the browser is closed—session cookies are not
    /// deleted and they will still be available when the browser is opened again.
    /// Check the documentation of the browsers you are targeting for up-to-date information.
    BrowserSession(BrowserSession),

    /// The session cookie will be a [persistent cookie].
    ///
    /// Persistent cookies have a pre-determined lifetime, specified via the `Max-Age` or `Expires`
    /// attribute. They do not disappear when the current browser session ends.
    ///
    /// [persistent cookie]: https://www.whitehatsec.com/glossary/content/persistent-session-cookie
    PersistentSession(PersistentSession),
}

/// A [session lifecycle](SessionLifecycle) strategy where the session cookie expires when the
/// browser's current session ends.
///
/// When does a browser session end? It depends on the browser. Chrome, for example, will often
/// continue running in the background when the browser is closed—session cookies are not deleted
/// and they will still be available when the browser is opened again. Check the documentation of
/// the browsers you are targeting for up-to-date information.
///
/// Due to its `Into<SessionLifecycle>` implementation, a `BrowserSession` can be passed directly
/// to [`SessionMiddlewareBuilder::session_lifecycle()`].
#[derive(Debug, Clone)]
pub struct BrowserSession {
    state_ttl: Duration,
    state_ttl_extension_policy: TtlExtensionPolicy,
}

impl BrowserSession {
    /// Sets a time-to-live (TTL) when storing the session state in the storage backend.
    ///
    /// We do not want to store session states indefinitely, otherwise we will inevitably run out of
    /// storage by holding on to the state of countless abandoned or expired sessions!
    ///
    /// We are dealing with the lifecycle of two uncorrelated object here: the session cookie
    /// and the session state. It is not a big issue if the session state outlives the cookie—
    /// we are wasting some space in the backend storage, but it will be cleaned up eventually.
    /// What happens, instead, if the cookie outlives the session state? A new session starts—
    /// e.g. if sessions are being used for authentication, the user is de-facto logged out.
    ///
    /// It is not possible to predict with certainty how long a browser session is going to
    /// last—you need to provide a reasonable upper bound. You do so via `state_ttl`—it dictates
    /// what TTL should be used for session state when the lifecycle of the session cookie is
    /// tied to the browser session length. [`SessionMiddleware`] will default to 1 day if
    /// `state_ttl` is left unspecified.
    ///
    /// You can mitigate the risk of the session cookie outliving the session state by
    /// specifying a more aggressive state TTL extension policy - check out
    /// [`BrowserSession::state_ttl_extension_policy`] for more details.
    pub fn state_ttl(mut self, ttl: Duration) -> Self {
        self.state_ttl = ttl;
        self
    }

    /// Determine under what circumstances the TTL of your session state should be extended.
    ///
    /// Defaults to [`TtlExtensionPolicy::OnStateChanges`] if left unspecified.
    ///
    /// See [`TtlExtensionPolicy`] for more details.
    pub fn state_ttl_extension_policy(mut self, ttl_extension_policy: TtlExtensionPolicy) -> Self {
        self.state_ttl_extension_policy = ttl_extension_policy;
        self
    }
}

impl Default for BrowserSession {
    fn default() -> Self {
        Self {
            state_ttl: default_ttl(),
            state_ttl_extension_policy: default_ttl_extension_policy(),
        }
    }
}

/// A [session lifecycle](SessionLifecycle) strategy where the session cookie will be [persistent].
///
/// Persistent cookies have a pre-determined expiration, specified via the `Max-Age` or `Expires`
/// attribute. They do not disappear when the current browser session ends.
///
/// Due to its `Into<SessionLifecycle>` implementation, a `PersistentSession` can be passed directly
/// to [`SessionMiddlewareBuilder::session_lifecycle()`].
///
/// # Examples
/// ```
/// use actix_web::cookie::time::Duration;
/// use actix_session::SessionMiddleware;
/// use actix_session::config::{PersistentSession, TtlExtensionPolicy};
///
/// const SECS_IN_WEEK: i64 = 60 * 60 * 24 * 7;
///
/// // a session lifecycle with a time-to-live (expiry) of 1 week and default extension policy
/// PersistentSession::default().session_ttl(Duration::seconds(SECS_IN_WEEK));
///
/// // a session lifecycle with the default time-to-live (expiry) and a custom extension policy
/// PersistentSession::default()
///     // this policy causes the session state's TTL to be refreshed on every request
///     .session_ttl_extension_policy(TtlExtensionPolicy::OnEveryRequest);
/// ```
///
/// [persistent]: https://www.whitehatsec.com/glossary/content/persistent-session-cookie
#[derive(Debug, Clone)]
pub struct PersistentSession {
    session_ttl: Duration,
    ttl_extension_policy: TtlExtensionPolicy,
}

impl PersistentSession {
    /// Specifies how long the session cookie should live.
    ///
    /// The session TTL is also used as the TTL for the session state in the storage backend.
    ///
    /// Defaults to 1 day.
    ///
    /// A persistent session can live more than the specified TTL if the TTL is extended.
    /// See [`session_ttl_extension_policy`](Self::session_ttl_extension_policy) for more details.
    #[doc(alias = "max_age", alias = "max age", alias = "expires")]
    pub fn session_ttl(mut self, session_ttl: Duration) -> Self {
        self.session_ttl = session_ttl;
        self
    }

    /// Determines under what circumstances the TTL of your session should be extended.
    /// See [`TtlExtensionPolicy`] for more details.
    ///
    /// Defaults to [`TtlExtensionPolicy::OnStateChanges`].
    pub fn session_ttl_extension_policy(
        mut self,
        ttl_extension_policy: TtlExtensionPolicy,
    ) -> Self {
        self.ttl_extension_policy = ttl_extension_policy;
        self
    }
}

impl Default for PersistentSession {
    fn default() -> Self {
        Self {
            session_ttl: default_ttl(),
            ttl_extension_policy: default_ttl_extension_policy(),
        }
    }
}

/// Configuration for which events should trigger an extension of the time-to-live for your session.
///
/// If you are using a [`BrowserSession`], `TtlExtensionPolicy` controls how often the TTL of the
/// session state should be refreshed. The browser is in control of the lifecycle of the session
/// cookie.
///
/// If you are using a [`PersistentSession`], `TtlExtensionPolicy` controls both the expiration of
/// the session cookie and the TTL of the session state on the storage backend.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TtlExtensionPolicy {
    /// The TTL is refreshed every time the server receives a request associated with a session.
    ///
    /// # Performance impact
    /// Refreshing the TTL on every request is not free. It implies a refresh of the TTL on the
    /// session state. This translates into a request over the network if you are using a remote
    /// system as storage backend (e.g. Redis). This impacts both the total load on your storage
    /// backend (i.e. number of queries it has to handle) and the latency of the requests served by
    /// your server.
    OnEveryRequest,

    /// The TTL is refreshed every time the session state changes or the session key is renewed.
    OnStateChanges,
}

/// Determines how to secure the content of the session cookie.
///
/// Used by [`SessionMiddlewareBuilder::cookie_content_security`].
#[derive(Debug, Clone, Copy)]
pub enum CookieContentSecurity {
    /// The cookie content is encrypted when using `CookieContentSecurity::Private`.
    ///
    /// Encryption guarantees confidentiality and integrity: the client cannot tamper with the
    /// cookie content nor decode it, as long as the encryption key remains confidential.
    Private,

    /// The cookie content is signed when using `CookieContentSecurity::Signed`.
    ///
    /// Signing guarantees integrity, but it doesn't ensure confidentiality: the client cannot
    /// tamper with the cookie content, but they can read it.
    Signed,
}

pub(crate) const fn default_ttl() -> Duration {
    Duration::days(1)
}

pub(crate) const fn default_ttl_extension_policy() -> TtlExtensionPolicy {
    TtlExtensionPolicy::OnStateChanges
}

/// A fluent, customized [`SessionMiddleware`] builder.
#[must_use]
pub struct SessionMiddlewareBuilder<Store: SessionStore> {
    storage_backend: Store,
    configuration: Configuration,
}

impl<Store: SessionStore> SessionMiddlewareBuilder<Store> {
    pub(crate) fn new(store: Store, configuration: Configuration) -> Self {
        Self {
            storage_backend: store,
            configuration,
        }
    }

    /// Set the name of the cookie used to store the session ID.
    ///
    /// Defaults to `id`.
    pub fn cookie_name(mut self, name: String) -> Self {
        self.configuration.cookie.name = name;
        self
    }

    /// Set the `Secure` attribute for the cookie used to store the session ID.
    ///
    /// If the cookie is set as secure, it will only be transmitted when the connection is secure
    /// (using `https`).
    ///
    /// Default is `true`.
    pub fn cookie_secure(mut self, secure: bool) -> Self {
        self.configuration.cookie.secure = secure;
        self
    }

    /// Determines what type of session cookie should be used and how its lifecycle should be managed.
    /// Check out [`SessionLifecycle`]'s documentation for more details on the available options.
    ///
    /// Default is [`SessionLifecycle::BrowserSession`].
    ///
    /// # Examples
    /// ```
    /// use actix_web::cookie::{Key, time::Duration};
    /// use actix_session::{SessionMiddleware, config::PersistentSession};
    /// use actix_session::storage::CookieSessionStore;
    ///
    /// const SECS_IN_WEEK: i64 = 60 * 60 * 24 * 7;
    ///
    /// // creates a session middleware with a time-to-live (expiry) of 1 week
    /// SessionMiddleware::builder(CookieSessionStore::default(), Key::from(&[0; 64]))
    ///     .session_lifecycle(
    ///         PersistentSession::default().session_ttl(Duration::seconds(SECS_IN_WEEK))
    ///     )
    ///     .build();
    /// ```
    pub fn session_lifecycle<S: Into<SessionLifecycle>>(mut self, session_lifecycle: S) -> Self {
        match session_lifecycle.into() {
            SessionLifecycle::BrowserSession(BrowserSession {
                state_ttl,
                state_ttl_extension_policy,
            }) => {
                self.configuration.cookie.max_age = None;
                self.configuration.session.state_ttl = state_ttl;
                self.configuration.ttl_extension_policy = state_ttl_extension_policy;
            }
            SessionLifecycle::PersistentSession(PersistentSession {
                session_ttl,
                ttl_extension_policy,
            }) => {
                self.configuration.cookie.max_age = Some(session_ttl);
                self.configuration.session.state_ttl = session_ttl;
                self.configuration.ttl_extension_policy = ttl_extension_policy;
            }
        }

        self
    }

    /// Set the `SameSite` attribute for the cookie used to store the session ID.
    ///
    /// By default, the attribute is set to `Lax`.
    pub fn cookie_same_site(mut self, same_site: SameSite) -> Self {
        self.configuration.cookie.same_site = same_site;
        self
    }

    /// Set the `Path` attribute for the cookie used to store the session ID.
    ///
    /// By default, the attribute is set to `/`.
    pub fn cookie_path(mut self, path: String) -> Self {
        self.configuration.cookie.path = path;
        self
    }

    /// Set the `Domain` attribute for the cookie used to store the session ID.
    ///
    /// Use `None` to leave the attribute unspecified. If unspecified, the attribute defaults
    /// to the same host that set the cookie, excluding subdomains.
    ///
    /// By default, the attribute is left unspecified.
    pub fn cookie_domain(mut self, domain: Option<String>) -> Self {
        self.configuration.cookie.domain = domain;
        self
    }

    /// Choose how the session cookie content should be secured.
    ///
    /// - [`CookieContentSecurity::Private`] selects encrypted cookie content.
    /// - [`CookieContentSecurity::Signed`] selects signed cookie content.
    ///
    /// # Default
    /// By default, the cookie content is encrypted. Encrypted was chosen instead of signed as
    /// default because it reduces the chances of sensitive information being exposed in the session
    /// key by accident, regardless of [`SessionStore`] implementation you chose to use.
    ///
    /// For example, if you are using cookie-based storage, you definitely want the cookie content
    /// to be encrypted—the whole session state is embedded in the cookie! If you are using
    /// Redis-based storage, signed is more than enough - the cookie content is just a unique
    /// tamper-proof session key.
    pub fn cookie_content_security(mut self, content_security: CookieContentSecurity) -> Self {
        self.configuration.cookie.content_security = content_security;
        self
    }

    /// Set the `HttpOnly` attribute for the cookie used to store the session ID.
    ///
    /// If the cookie is set as `HttpOnly`, it will not be visible to any JavaScript snippets
    /// running in the browser.
    ///
    /// Default is `true`.
    pub fn cookie_http_only(mut self, http_only: bool) -> Self {
        self.configuration.cookie.http_only = http_only;
        self
    }

    /// Finalise the builder and return a [`SessionMiddleware`] instance.
    #[must_use]
    pub fn build(self) -> SessionMiddleware<Store> {
        SessionMiddleware::from_parts(self.storage_backend, self.configuration)
    }
}

#[derive(Clone)]
pub(crate) struct Configuration {
    pub(crate) cookie: CookieConfiguration,
    pub(crate) session: SessionConfiguration,
    pub(crate) ttl_extension_policy: TtlExtensionPolicy,
}

#[derive(Clone)]
pub(crate) struct SessionConfiguration {
    pub(crate) state_ttl: Duration,
}

#[derive(Clone)]
pub(crate) struct CookieConfiguration {
    pub(crate) secure: bool,
    pub(crate) http_only: bool,
    pub(crate) name: String,
    pub(crate) same_site: SameSite,
    pub(crate) path: String,
    pub(crate) domain: Option<String>,
    pub(crate) max_age: Option<Duration>,
    pub(crate) content_security: CookieContentSecurity,
    pub(crate) key: Key,
}

pub(crate) fn default_configuration(key: Key) -> Configuration {
    Configuration {
        cookie: CookieConfiguration {
            secure: true,
            http_only: true,
            name: "id".into(),
            same_site: SameSite::Lax,
            path: "/".into(),
            domain: None,
            max_age: None,
            content_security: CookieContentSecurity::Private,
            key,
        },
        session: SessionConfiguration {
            state_ttl: default_ttl(),
        },
        ttl_extension_policy: default_ttl_extension_policy(),
    }
}
