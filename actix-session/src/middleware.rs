use crate::storage::SessionStore;
use crate::{Session, SessionStatus};
use actix_web::body::MessageBody;
use actix_web::cookie::{Cookie, CookieJar, Key, SameSite};
use actix_web::dev::{ResponseHead, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderValue, SET_COOKIE};
use actix_web::HttpRequest;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use time::Duration;

/// ## How did we choose defaults?
///
/// If you add `actix-session` to your dependencies and go to production using the default
/// configuration you should not have to regret it.
/// That is why, when in doubt, we opt to use the most secure option for each configuration
/// parameter.
/// We expose knobs to change the default to suit your needs - i.e. if you know
/// what you are doing, we will not stop you. But being a subject-matter expert should not
/// be a requirement to deploy a reasonably secure implementation of sessions.
pub struct SessionMiddleware<Store: SessionStore> {
    storage_backend: Arc<Store>,
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

/// Used by [`SessionMiddlewareBuilder::session_length`] to determine how long a session
/// should last.
#[derive(Clone, Debug)]
pub enum SessionLength {
    /// The session cookie will expire when the current browser session ends.
    ///
    /// When does a browser session end? It depends on the browser!
    /// Chrome, for example, will often continue running in the background when the
    /// browser is closed - session cookies are not deleted and they will still
    /// be available when the browser is opened again.
    /// Check the documentation of the browser you are targeting for up-to-date information.
    BrowserSession {
        /// We must provide a time-to-live (TTL) when storing the session state
        /// in the storage backend - we do not want to store session states indefinitely,
        /// otherwise we will inevitably run out of storage by holding on to the state
        /// of countless abandoned or expired sessions!
        ///
        /// We are dealing with the lifecycle of two uncorrelated object here: the session
        /// cookie and the session state.
        /// It is not a big issue if the session state outlives the cookie - we are wasting
        /// some space in the backend storage, but it will be cleaned up eventually.
        /// What happens, instead, if the cookie outlives the session state?
        /// A new session starts - e.g. if sessions are being used for authentication,
        /// the user is de-facto logged out.
        ///
        /// It is not possible to predict with certainty how long a browser session
        /// is going to last - you need to provide a reasonable upper bound.
        /// You do so via `state_ttl` - it dictates what TTL should be used for session
        /// state when the lifecycle of the session cookie is tied to the browser session
        /// length.
        /// [`SessionMiddleware`] will default to 1 day if `state_ttl` is left unspecified.
        state_ttl: Option<Duration>,
    },
    /// The session cookie will be a [persistent cookie](https://www.whitehatsec.com/glossary/content/persistent-session-cookie).
    /// Persistent cookies have a pre-determined lifetime, specified via the `Max-Age` or
    /// `Expires` attribute. They do not disappear when the current browser session ends.
    Predetermined {
        /// Set `max_session_length` to specify how long the session cookie should live.
        /// [`SessionMiddleware`] will default to 1 day if `max_session_length` is set to `None`.
        ///
        /// `max_session_length` is also used as the TTL for the session state in the
        /// storage backend.
        max_session_length: Option<Duration>,
    },
}

#[derive(Copy, Clone, Debug)]
/// Used by [`SessionMiddlewareBuilder::cookie_content_security`] to determine how to secure
/// the content of the session cookie.
pub enum CookieContentSecurity {
    /// `CookieContentSecurity::Signed` translates into a signed cookie content - i.e.
    /// the end-user/JavaScript scripts cannot tamper with its content, but they can read it
    /// (i.e. no confidentiality).
    Signed,
    /// `CookieContentSecurity::Private` translates into an encrypted cookie content - i.e.
    /// the end-user cannot/JavaScript scripts cannot tamper with its content nor decode it
    /// (i.e. it preserves confidentiality, as long the as the encryption key is not breached).
    Private,
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
    pub fn new(store: Store, key: Key) -> Self {
        Self {
            storage_backend: Arc::new(store),
            configuration: Rc::new(default_configuration(key)),
        }
    }

    #[must_use]
    pub fn builder(store: Store, key: Key) -> SessionMiddlewareBuilder<Store> {
        SessionMiddlewareBuilder {
            storage_backend: Arc::new(store),
            configuration: default_configuration(key),
        }
    }
}

#[must_use]
pub struct SessionMiddlewareBuilder<Store: SessionStore> {
    storage_backend: Arc<Store>,
    configuration: Configuration,
}

impl<Store: SessionStore> SessionMiddlewareBuilder<Store> {
    /// Set the name of the cookie used to store the session id.
    ///
    /// Defaults to `id`.
    #[must_use]
    pub fn cookie_name(mut self, name: String) -> Self {
        self.configuration.cookie.name = name;
        self
    }

    /// Set the `Secure` attribute for the cookie used to store the session id.
    ///
    /// If the cookie is set as secure, it will only be transmitted when the
    /// connection is secure - i.e. `https`.
    ///
    /// Default is `true`.
    #[must_use]
    pub fn cookie_secure(mut self, secure: bool) -> Self {
        self.configuration.cookie.secure = secure;
        self
    }

    /// Determine how long a session should last - check out [`CookieDurability`]'s documentation
    /// for more details on the available options.
    ///
    /// Default is [`CookieDurability::BrowserSession`].
    #[must_use]
    pub fn session_length(mut self, session_length: SessionLength) -> Self {
        match session_length {
            SessionLength::BrowserSession { state_ttl } => {
                self.configuration.cookie.max_age = None;
                self.configuration.session.state_ttl = state_ttl.unwrap_or_else(default_ttl);
            }
            SessionLength::Predetermined { max_session_length } => {
                let ttl = max_session_length.unwrap_or_else(default_ttl);
                self.configuration.cookie.max_age = Some(ttl.clone());
                self.configuration.session.state_ttl = ttl;
            }
        }
        self
    }

    /// Set the `SameSite` attribute for the cookie used to store the session id.
    ///
    /// By default, the attribute is set to `Lax`.
    #[must_use]
    pub fn cookie_same_site(mut self, same_site: SameSite) -> Self {
        self.configuration.cookie.same_site = same_site;
        self
    }

    /// Set the `Path` attribute for the cookie used to store the session id.
    ///
    /// By default, the attribute is set to `/`.
    #[must_use]
    pub fn cookie_path(mut self, path: String) -> Self {
        self.configuration.cookie.path = path;
        self
    }

    /// Set the `Domain` attribute for the cookie used to store the session id.
    ///
    /// Use `None` to leave the attribute unspecified. If unspecified, the attribute defaults
    /// to the same host that set the cookie, excluding subdomains.
    ///
    /// By default, the attribute is left unspecified.
    #[must_use]
    pub fn cookie_domain(mut self, domain: Option<String>) -> Self {
        self.configuration.cookie.domain = domain;
        self
    }

    /// Choose how the session cookie content should be secured.
    ///
    /// `CookieContentSecurity::Private` translates into an encrypted cookie content.
    /// `CookieContentSecurity::Signed` translates into a signed cookie content.
    ///
    /// ## Default
    ///
    /// By default, the cookie content is encrypted.
    /// We choose encrypted instead of signed as default because it reduces the chances of
    /// sensitive information being exposed in the session key by accident, regardless of
    /// [`SessionStore`] implementation you chose to use.
    ///
    /// E.g. if you are using cookie-based storage, you definitely want the cookie content
    /// to be encrypted - the whole session state is embedded in the cookie!
    /// If you are using Redis-based storage, signed is more than enough - the cookie content
    /// is just a unique tamper-proof session key.
    #[must_use]
    pub fn cookie_content_security(mut self, content_security: CookieContentSecurity) -> Self {
        self.configuration.cookie.content_security = content_security;
        self
    }

    /// Set the `HttpOnly` attribute for the cookie used to store the session id.
    ///
    /// If the cookie is set as `HttpOnly`, it will not be visible to any JavaScript
    /// snippets running in the browser.
    ///
    /// Default is `true`.
    #[must_use]
    pub fn cookie_http_only(mut self, http_only: bool) -> Self {
        self.configuration.cookie.http_only = http_only;
        self
    }

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
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(InnerSessionMiddleware {
            service: Rc::new(service),
            configuration: Rc::clone(&self.configuration),
            storage_backend: self.storage_backend.clone(),
        }))
    }
}

#[non_exhaustive]
#[doc(hidden)]
pub struct InnerSessionMiddleware<S, Store: SessionStore + 'static> {
    service: Rc<S>,
    configuration: Rc<Configuration>,
    storage_backend: Arc<Store>,
}

#[allow(clippy::type_complexity)]
impl<S, B, Store> Service<ServiceRequest> for InnerSessionMiddleware<S, Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let storage_backend = self.storage_backend.clone();
        let configuration = Rc::clone(&self.configuration);

        Box::pin(async move {
            let (request, payload) = req.into_parts();
            let session_key = extract_session_key(&request, &configuration.cookie);
            let session_state = if let Some(session_key) = session_key.as_ref() {
                // TODO: remove unwrap
                storage_backend
                    .load(&session_key)
                    .await
                    .unwrap()
                    .unwrap_or_default()
            } else {
                HashMap::new()
            };
            let mut req = ServiceRequest::from_parts(request, payload);
            Session::set_session(&mut req, session_state);

            let mut res = service.call(req).await?;
            let (status, state) = Session::get_changes(&mut res);
            let session_state: HashMap<String, String> = state.collect();
            match session_key {
                None => {
                    // We do not create an entry in the session store if there is no state
                    // attached to a fresh session
                    if !session_state.is_empty() {
                        // TODO: remove unwrap
                        let session_key = storage_backend.save(session_state).await.unwrap();
                        set_session_cookie(
                            res.response_mut().head_mut(),
                            session_key,
                            &configuration.cookie,
                        );
                    }
                }
                Some(session_key) => {
                    match status {
                        SessionStatus::Changed => {
                            // TODO: remove unwrap
                            let session_key = storage_backend
                                .update(session_key, session_state)
                                .await
                                .unwrap();
                            set_session_cookie(
                                res.response_mut().head_mut(),
                                session_key,
                                &configuration.cookie,
                            );
                        }
                        SessionStatus::Purged => {
                            // TODO: remove unwrap
                            storage_backend.delete(&session_key).await.unwrap();
                            delete_session_cookie(
                                res.response_mut().head_mut(),
                                &configuration.cookie,
                            );
                        }
                        SessionStatus::Renewed => {
                            // TODO: remove unwrap
                            storage_backend.delete(&session_key).await.unwrap();
                            // TODO: remove unwrap
                            let session_key = storage_backend.save(session_state).await.unwrap();
                            set_session_cookie(
                                res.response_mut().head_mut(),
                                session_key,
                                &configuration.cookie,
                            );
                        }
                        SessionStatus::Unchanged => {
                            // Nothing to do - we avoid the unnecessary call to the storage
                        }
                    }
                }
            };
            Ok(res)
        })
    }
}

fn extract_session_key(req: &HttpRequest, config: &CookieConfiguration) -> Option<String> {
    let cookies = req.cookies().ok()?;
    let session_cookie = cookies
        .iter()
        .find(|&cookie| cookie.name() == config.name)?;

    let mut jar = CookieJar::new();
    jar.add_original(session_cookie.clone());

    let verified_cookie = match config.content_security {
        CookieContentSecurity::Signed => jar.signed(&config.key).get(&config.name),
        CookieContentSecurity::Private => jar.private(&config.key).get(&config.name),
    }?;
    Some(verified_cookie.value().to_owned())
}

fn set_session_cookie(
    response: &mut ResponseHead,
    session_key: String,
    config: &CookieConfiguration,
) {
    let mut cookie = Cookie::new(config.name.clone(), session_key);
    cookie.set_secure(config.secure);
    cookie.set_http_only(config.http_only);
    if let Some(max_age) = config.max_age {
        cookie.set_max_age(max_age);
    }
    cookie.set_same_site(config.same_site);
    cookie.set_path(config.path.clone());

    let mut jar = CookieJar::new();
    match config.content_security {
        CookieContentSecurity::Signed => jar.signed_mut(&config.key).add(cookie),
        CookieContentSecurity::Private => jar.private_mut(&config.key).add(cookie),
    }

    // Set cookie
    let cookie = jar.delta().next().unwrap();
    // TODO: remove unwrap
    let val = HeaderValue::from_str(&cookie.encoded().to_string()).unwrap();
    response.headers_mut().append(SET_COOKIE, val);
}

fn delete_session_cookie(response: &mut ResponseHead, config: &CookieConfiguration) {
    let removal_cookie = Cookie::build(config.name.clone(), "")
        .max_age(time::Duration::seconds(0))
        .finish();
    // TODO: remove unwrap
    let val = HeaderValue::from_str(&removal_cookie.to_string()).unwrap();
    response.headers_mut().append(SET_COOKIE, val);
}
