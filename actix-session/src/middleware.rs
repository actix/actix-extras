use std::{collections::HashMap, convert::TryInto, fmt, future::Future, pin::Pin, rc::Rc};

use actix_utils::future::{ready, Ready};
use actix_web::{
    body::MessageBody,
    cookie::{Cookie, CookieJar, Key, SameSite},
    dev::{forward_ready, ResponseHead, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderValue, SET_COOKIE},
};
use anyhow::Context;
use time::Duration;

use crate::{
    storage::{LoadError, SessionKey, SessionStore},
    Session, SessionStatus,
};

/// A middleware for session management in Actix Web applications.
///
/// [`SessionMiddleware`] takes care of a few jobs:
///
/// - Instructs the session storage backend to create/update/delete/retrieve the state attached to
///   a session according to its status and the operations that have been performed against it;
/// - Set/remove a cookie, on the client side, to enable a user to be consistently associated with
///   the same session across multiple HTTP requests.
///
/// Use [`SessionMiddleware::new`] to initialize the session framework using the default parameters.
/// To create a new instance of [`SessionMiddleware`] you need to provide:
///
/// - an instance of the session storage backend you wish to use (i.e. an implementation of
///   [`SessionStore]);
/// - a secret key, to sign or encrypt the content of client-side session cookie.
///
/// ```no_run
/// use actix_web::{web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{Session, SessionMiddleware, storage::RedisActorSessionStore};
/// use actix_web::cookie::Key;
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
///     let redis_connection_string = "127.0.0.1:6379";
///     HttpServer::new(move ||
///             App::new()
///             // Add session management to your application using Redis for session state storage
///             .wrap(
///                 SessionMiddleware::new(
///                     RedisActorSessionStore::new(redis_connection_string),
///                     secret_key.clone()
///                 )
///             )
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// If you want to customise use [`builder`](Self::builder) instead of [`new`](Self::new):
///
/// ```no_run
/// use actix_web::{cookie::Key, web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{Session, SessionMiddleware, storage::RedisActorSessionStore, SessionLength};
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
///     let redis_connection_string = "127.0.0.1:6379";
///     HttpServer::new(move ||
///             App::new()
///             // Customise session length!
///             .wrap(
///                 SessionMiddleware::builder(
///                     RedisActorSessionStore::new(redis_connection_string),
///                     secret_key.clone()
///                 )
///                 .session_length(SessionLength::Predetermined {
///                     max_session_length: Some(time::Duration::days(5)),
///                 })
///                 .build(),
///             )
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// ## How did we choose defaults?
///
/// You should not regret adding `actix-session` to your dependencies and going to production using
/// the default configuration. That is why, when in doubt, we opt to use the most secure option for
/// each configuration parameter.
///
/// We expose knobs to change the default to suit your needs—i.e., if you know what you are doing,
/// we will not stop you. But being a subject-matter expert should not be a requirement to deploy
/// reasonably secure implementation of sessions.
#[derive(Clone)]
pub struct SessionMiddleware<Store: SessionStore> {
    storage_backend: Rc<Store>,
    configuration: Rc<Configuration>,
}

#[derive(Clone)]
struct Configuration {
    cookie: CookieConfiguration,
    session: SessionConfiguration,
}

#[derive(Clone)]
struct SessionConfiguration {
    state_ttl: Duration,
}

#[derive(Clone)]
struct CookieConfiguration {
    secure: bool,
    http_only: bool,
    name: String,
    same_site: SameSite,
    path: String,
    domain: Option<String>,
    max_age: Option<Duration>,
    content_security: CookieContentSecurity,
    key: Key,
}

/// Describes how long a session should last.
///
/// Used by [`SessionMiddlewareBuilder::session_length`].
#[derive(Clone, Debug)]
pub enum SessionLength {
    /// The session cookie will expire when the current browser session ends.
    ///
    /// When does a browser session end? It depends on the browser! Chrome, for example, will often
    /// continue running in the background when the browser is closed—session cookies are not
    /// deleted and they will still be available when the browser is opened again. Check the
    /// documentation of the browser you are targeting for up-to-date information.
    BrowserSession {
        /// We must provide a time-to-live (TTL) when storing the session state in the storage
        /// backend—we do not want to store session states indefinitely, otherwise we will
        /// inevitably run out of storage by holding on to the state of countless abandoned or
        /// expired sessions!
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
        state_ttl: Option<Duration>,
    },

    /// The session cookie will be a [persistent cookie].
    ///
    /// Persistent cookies have a pre-determined lifetime, specified via the `Max-Age` or `Expires`
    /// attribute. They do not disappear when the current browser session ends.
    ///
    /// [persistent cookie]: https://www.whitehatsec.com/glossary/content/persistent-session-cookie
    Predetermined {
        /// Set `max_session_length` to specify how long the session cookie should live.
        /// [`SessionMiddleware`] will default to 1 day if `max_session_length` is set to `None`.
        ///
        /// `max_session_length` is also used as the TTL for the session state in the
        /// storage backend.
        max_session_length: Option<Duration>,
    },
}

/// Used by [`SessionMiddlewareBuilder::cookie_content_security`] to determine how to secure
/// the content of the session cookie.
#[derive(Debug, Clone, Copy)]
pub enum CookieContentSecurity {
    /// `CookieContentSecurity::Private` selects encrypted cookie content.
    ///
    /// The client cannot tamper with its contents nor decode it (i.e., preserves confidentiality as
    /// long the as the encryption key is not breached).
    Private,

    /// `CookieContentSecurity::Signed` selects signed cookie content.
    ///
    /// The client cannot tamper with its contents, but they can read it (i.e., no confidentiality).
    Signed,
}

fn default_configuration(key: Key) -> Configuration {
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
    }
}

fn default_ttl() -> Duration {
    Duration::days(1)
}

impl<Store: SessionStore> SessionMiddleware<Store> {
    /// Use [`SessionMiddleware::new`] to initialize the session framework using the default
    /// parameters.
    ///
    /// To create a new instance of [`SessionMiddleware`] you need to provide:
    /// - an instance of the session storage backend you wish to use (i.e. an implementation of
    ///   [`SessionStore]);
    /// - a secret key, to sign or encrypt the content of client-side session cookie.
    pub fn new(store: Store, key: Key) -> Self {
        Self {
            storage_backend: Rc::new(store),
            configuration: Rc::new(default_configuration(key)),
        }
    }

    /// A fluent API to configure [`SessionMiddleware`].
    ///
    /// It takes as input the two required inputs to create a new instance of [`SessionMiddleware`]:
    /// - an instance of the session storage backend you wish to use (i.e. an implementation of
    ///   [`SessionStore]);
    /// - a secret key, to sign or encrypt the content of client-side session cookie.
    pub fn builder(store: Store, key: Key) -> SessionMiddlewareBuilder<Store> {
        SessionMiddlewareBuilder {
            storage_backend: Rc::new(store),
            configuration: default_configuration(key),
        }
    }
}

/// A fluent builder to construct a [`SessionMiddleware`] instance with custom configuration
/// parameters.
#[must_use]
pub struct SessionMiddlewareBuilder<Store: SessionStore> {
    storage_backend: Rc<Store>,
    configuration: Configuration,
}

impl<Store: SessionStore> SessionMiddlewareBuilder<Store> {
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

    /// Determine how long a session should last - check out [`SessionLength`]'s documentation for
    /// more details on the available options.
    ///
    /// Default is [`SessionLength::BrowserSession`].
    pub fn session_length(mut self, session_length: SessionLength) -> Self {
        match session_length {
            SessionLength::BrowserSession { state_ttl } => {
                self.configuration.cookie.max_age = None;
                self.configuration.session.state_ttl = state_ttl.unwrap_or_else(default_ttl);
            }
            SessionLength::Predetermined { max_session_length } => {
                let ttl = max_session_length.unwrap_or_else(default_ttl);
                self.configuration.cookie.max_age = Some(ttl);
                self.configuration.session.state_ttl = ttl;
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
        SessionMiddleware {
            storage_backend: self.storage_backend,
            configuration: Rc::new(self.configuration),
        }
    }
}

impl<S, B, Store> Transform<S, ServiceRequest> for SessionMiddleware<Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = InnerSessionMiddleware<S, Store>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(InnerSessionMiddleware {
            service: Rc::new(service),
            configuration: Rc::clone(&self.configuration),
            storage_backend: Rc::clone(&self.storage_backend),
        }))
    }
}

/// Short-hand to create an `actix_web::Error` instance that will result in an `Internal Server
/// Error` response while preserving the error root cause (e.g. in logs).
fn e500<E: fmt::Debug + fmt::Display + 'static>(err: E) -> actix_web::Error {
    actix_web::error::ErrorInternalServerError(err)
}

#[doc(hidden)]
#[non_exhaustive]
pub struct InnerSessionMiddleware<S, Store: SessionStore + 'static> {
    service: Rc<S>,
    configuration: Rc<Configuration>,
    storage_backend: Rc<Store>,
}

impl<S, B, Store> Service<ServiceRequest> for InnerSessionMiddleware<S, Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let storage_backend = Rc::clone(&self.storage_backend);
        let configuration = Rc::clone(&self.configuration);

        Box::pin(async move {
            let session_key = extract_session_key(&req, &configuration.cookie);
            let (session_key, session_state) =
                load_session_state(session_key, storage_backend.as_ref()).await?;

            Session::set_session(&mut req, session_state);

            let mut res = service.call(req).await?;
            let (status, session_state) = Session::get_changes(&mut res);

            match session_key {
                None => {
                    // we do not create an entry in the session store if there is no state attached
                    // to a fresh session
                    if !session_state.is_empty() {
                        let session_key = storage_backend
                            .save(session_state, &configuration.session.state_ttl)
                            .await
                            .map_err(e500)?;

                        set_session_cookie(
                            res.response_mut().head_mut(),
                            session_key,
                            &configuration.cookie,
                        )
                        .map_err(e500)?;
                    }
                }

                Some(session_key) => {
                    match status {
                        SessionStatus::Changed => {
                            let session_key = storage_backend
                                .update(
                                    session_key,
                                    session_state,
                                    &configuration.session.state_ttl,
                                )
                                .await
                                .map_err(e500)?;

                            set_session_cookie(
                                res.response_mut().head_mut(),
                                session_key,
                                &configuration.cookie,
                            )
                            .map_err(e500)?;
                        }

                        SessionStatus::Purged => {
                            storage_backend.delete(&session_key).await.map_err(e500)?;

                            delete_session_cookie(
                                res.response_mut().head_mut(),
                                &configuration.cookie,
                            )
                            .map_err(e500)?;
                        }

                        SessionStatus::Renewed => {
                            storage_backend.delete(&session_key).await.map_err(e500)?;

                            let session_key = storage_backend
                                .save(session_state, &configuration.session.state_ttl)
                                .await
                                .map_err(e500)?;

                            set_session_cookie(
                                res.response_mut().head_mut(),
                                session_key,
                                &configuration.cookie,
                            )
                            .map_err(e500)?;
                        }

                        SessionStatus::Unchanged => {
                            // Nothing to do; we avoid the unnecessary call to the storage.
                        }
                    }
                }
            };
            Ok(res)
        })
    }
}

fn extract_session_key(req: &ServiceRequest, config: &CookieConfiguration) -> Option<SessionKey> {
    let cookies = req.cookies().ok()?;
    let session_cookie = cookies
        .iter()
        .find(|&cookie| cookie.name() == config.name)?;

    let mut jar = CookieJar::new();
    jar.add_original(session_cookie.clone());

    let verification_result = match config.content_security {
        CookieContentSecurity::Signed => jar.signed(&config.key).get(&config.name),
        CookieContentSecurity::Private => jar.private(&config.key).get(&config.name),
    };

    if verification_result.is_none() {
        tracing::warn!(
            "The session cookie attached to the incoming request failed to pass cryptographic \
            checks (signature verification/decryption)."
        );
    }

    match verification_result?.value().to_owned().try_into() {
        Ok(session_key) => Some(session_key),
        Err(err) => {
            tracing::warn!(
                error.message = %err,
                error.cause_chain = ?err,
                "Invalid session key, ignoring."
            );

            None
        }
    }
}

async fn load_session_state<Store: SessionStore>(
    session_key: Option<SessionKey>,
    storage_backend: &Store,
) -> Result<(Option<SessionKey>, HashMap<String, String>), actix_web::Error> {
    if let Some(session_key) = session_key {
        match storage_backend.load(&session_key).await {
            Ok(state) => {
                if let Some(state) = state {
                    Ok((Some(session_key), state))
                } else {
                    // We discard the existing session key given that the state attached to it can
                    // no longer be found (e.g. it expired or we suffered some data loss in the
                    // storage). Regenerating the session key will trigger the `save` workflow
                    // instead of the `update` workflow if the session state is modified during the
                    // lifecycle of the current request.

                    tracing::info!(
                        "No session state has been found for a valid session key, creating a new \
                        empty session."
                    );

                    Ok((None, HashMap::new()))
                }
            }

            Err(err) => match err {
                LoadError::Deserialization(err) => {
                    tracing::warn!(
                        error.message = %err,
                        error.cause_chain = ?err,
                        "Invalid session state, creating a new empty session."
                    );

                    Ok((Some(session_key), HashMap::new()))
                }

                LoadError::Other(err) => Err(e500(err)),
            },
        }
    } else {
        Ok((None, HashMap::new()))
    }
}

fn set_session_cookie(
    response: &mut ResponseHead,
    session_key: SessionKey,
    config: &CookieConfiguration,
) -> Result<(), anyhow::Error> {
    let value: String = session_key.into();
    let mut cookie = Cookie::new(config.name.clone(), value);

    cookie.set_secure(config.secure);
    cookie.set_http_only(config.http_only);
    cookie.set_same_site(config.same_site);
    cookie.set_path(config.path.clone());

    if let Some(max_age) = config.max_age {
        cookie.set_max_age(max_age);
    }

    if let Some(ref domain) = config.domain {
        cookie.set_domain(domain.clone());
    }

    let mut jar = CookieJar::new();
    match config.content_security {
        CookieContentSecurity::Signed => jar.signed_mut(&config.key).add(cookie),
        CookieContentSecurity::Private => jar.private_mut(&config.key).add(cookie),
    }

    // set cookie
    let cookie = jar.delta().next().unwrap();
    let val = HeaderValue::from_str(&cookie.encoded().to_string())
        .context("Failed to attach a session cookie to the outgoing response")?;

    response.headers_mut().append(SET_COOKIE, val);

    Ok(())
}

fn delete_session_cookie(
    response: &mut ResponseHead,
    config: &CookieConfiguration,
) -> Result<(), anyhow::Error> {
    let removal_cookie = Cookie::build(config.name.clone(), "")
        .path(config.path.clone())
        .http_only(config.http_only);

    let mut removal_cookie = if let Some(ref domain) = config.domain {
        removal_cookie.domain(domain)
    } else {
        removal_cookie
    }
    .finish();

    removal_cookie.make_removal();

    let val = HeaderValue::from_str(&removal_cookie.to_string())
        .context("Failed to attach a session removal cookie to the outgoing response")?;
    response.headers_mut().append(SET_COOKIE, val);

    Ok(())
}
