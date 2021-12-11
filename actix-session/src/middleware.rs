use crate::storage::SessionStore;
use crate::{Session, SessionStatus};
use actix_web::body::MessageBody;
use actix_web::cookie::{Cookie, CookieJar, Key, SameSite};
use actix_web::dev::{ResponseHead, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::SET_COOKIE;
use actix_web::http::HeaderValue;
use actix_web::HttpRequest;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use time::Duration;

pub struct SessionMiddleware<Store: SessionStore> {
    storage_backend: Arc<Store>,
    cookie_configuration: Rc<SessionCookieConfiguration>,
}

#[derive(Clone)]
struct SessionCookieConfiguration {
    secure: bool,
    http_only: bool,
    name: String,
    same_site: SameSite,
    path: String,
    domain: Option<String>,
    // TODO: rename to cookie durability using enum
    max_age: Option<Duration>,
    content_security: CookieContentSecurity,
    key: Key,
}

#[derive(Copy, Clone)]
pub enum CookieContentSecurity {
    Signed,
    Private,
}

fn default_cookie_configuration(key: Key) -> SessionCookieConfiguration {
    SessionCookieConfiguration {
        secure: true,
        http_only: true,
        name: "id".into(),
        same_site: SameSite::Lax,
        path: "/".into(),
        domain: None,
        max_age: None,
        content_security: CookieContentSecurity::Signed,
        key,
    }
}

impl<Store: SessionStore> SessionMiddleware<Store> {
    pub fn new(store: Store, key: Key) -> Self {
        Self {
            storage_backend: Arc::new(store),
            cookie_configuration: Rc::new(default_cookie_configuration(key)),
        }
    }

    pub fn builder(store: Store, key: Key) -> SessionMiddlewareBuilder<Store> {
        SessionMiddlewareBuilder {
            storage_backend: Arc::new(store),
            cookie_configuration: default_cookie_configuration(key),
        }
    }
}

pub struct SessionMiddlewareBuilder<Store: SessionStore> {
    storage_backend: Arc<Store>,
    cookie_configuration: SessionCookieConfiguration,
}

impl<Store: SessionStore> SessionMiddlewareBuilder<Store> {
    /// Set the name of the cookie used to store the session id.
    ///
    /// Defaults to `id`.
    pub fn cookie_name(mut self, name: String) -> Self {
        self.cookie_configuration.name = name;
        self
    }

    /// Set the `Secure` attribute for the cookie used to store the session id.
    ///
    /// If the cookie is set as secure, it will only be transmitted when the
    /// connection is secure - i.e. `https`.
    ///
    /// Default is `true`.
    pub fn cookie_secure(mut self, secure: bool) -> Self {
        self.cookie_configuration.secure = secure;
        self
    }

    /// Set the `Max-Age` attribute for the cookie used to store the session id.
    ///
    /// Use `None` for session-only cookies.
    ///
    /// Default is `None`.
    pub fn cookie_max_age(mut self, max_age: Option<Duration>) -> Self {
        self.cookie_configuration.max_age = max_age;
        self
    }

    /// Set the `SameSite` attribute for the cookie used to store the session id.
    ///
    /// By default, the attribute is set to `Lax`.
    pub fn cookie_same_site(mut self, same_site: SameSite) -> Self {
        self.cookie_configuration.same_site = same_site;
        self
    }

    /// Set the `Path` attribute for the cookie used to store the session id.
    ///
    /// By default, the attribute is set to `/`.
    pub fn cookie_path(mut self, path: String) -> Self {
        self.cookie_configuration.path = path;
        self
    }

    /// Set the `Domain` attribute for the cookie used to store the session id.
    ///
    /// Use `None` to leave the attribute unspecified. If unspecified, the attribute defaults
    /// to the same host that set the cookie, excluding subdomains.
    ///
    /// By default, the attribute is left unspecified.
    pub fn cookie_domain(mut self, domain: Option<String>) -> Self {
        self.cookie_configuration.domain = domain;
        self
    }

    /// Choose how the session cookie content should be secured.
    ///
    /// `CookieContentSecurity::Private` translates into an encrypted cookie content.
    /// `CookieContentSecurity::Signed` translates into a signed cookie content.
    ///
    /// By default, the content is signed, not encrypted.
    pub fn cookie_content_security(mut self, content_security: CookieContentSecurity) -> Self {
        self.cookie_configuration.content_security = content_security;
        self
    }

    /// Set the `HttpOnly` attribute for the cookie used to store the session id.
    ///
    /// If the cookie is set as `HttpOnly`, it will not be visible to any JavaScript
    /// snippets running in the browser.
    ///
    /// Default is `true`.
    pub fn cookie_http_only(mut self, http_only: bool) -> Self {
        self.cookie_configuration.http_only = http_only;
        self
    }

    pub fn build(self) -> SessionMiddleware<Store> {
        SessionMiddleware {
            storage_backend: self.storage_backend,
            cookie_configuration: Rc::new(self.cookie_configuration),
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
            cookie_configuration: Rc::clone(&self.cookie_configuration),
            storage_backend: self.storage_backend.clone(),
        }))
    }
}

#[non_exhaustive]
#[doc(hidden)]
pub struct InnerSessionMiddleware<S, Store: SessionStore + 'static> {
    service: Rc<S>,
    cookie_configuration: Rc<SessionCookieConfiguration>,
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
        let cookie_configuration = Rc::clone(&self.cookie_configuration);

        Box::pin(async move {
            let (request, payload) = req.into_parts();
            let session_key = extract_session_key(
                &request,
                &cookie_configuration.key,
                &cookie_configuration.name,
            );
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
                            &cookie_configuration,
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
                                &cookie_configuration,
                            );
                        }
                        SessionStatus::Purged => {
                            // TODO: remove unwrap
                            storage_backend.delete(&session_key).await.unwrap();
                            delete_session_cookie(
                                res.response_mut().head_mut(),
                                &cookie_configuration,
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
                                &cookie_configuration,
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

fn extract_session_key(req: &HttpRequest, signing_key: &Key, cookie_name: &str) -> Option<String> {
    // TODO: Should we fail the request if we cannot read cookies?
    let cookies = req.cookies().ok()?;
    let session_cookie = cookies
        .iter()
        .find(|&cookie| cookie.name() == cookie_name)?;

    let mut jar = CookieJar::new();
    jar.add_original(session_cookie.clone());
    let verified_cookie = jar.signed(&signing_key).get(&cookie_name)?;
    Some(verified_cookie.value().to_owned())
}

fn set_session_cookie(
    response: &mut ResponseHead,
    session_key: String,
    config: &SessionCookieConfiguration,
) {
    let mut cookie = Cookie::new(config.name.clone(), session_key);
    cookie.set_secure(config.secure);
    cookie.set_http_only(config.http_only);
    if let Some(max_age) = config.max_age {
        cookie.set_max_age(max_age);
    }
    cookie.set_same_site(config.same_site);

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

fn delete_session_cookie(response: &mut ResponseHead, config: &SessionCookieConfiguration) {
    let removal_cookie = Cookie::build(config.name.clone(), "")
        .max_age(time::Duration::seconds(0))
        .finish();
    // TODO: remove unwrap
    let val = HeaderValue::from_str(&removal_cookie.to_string()).unwrap();
    response.headers_mut().append(SET_COOKIE, val);
}
