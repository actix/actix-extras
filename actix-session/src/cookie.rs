//! Cookie based sessions. See docs for [`CookieSession`].

use std::{collections::HashMap, error::Error as StdError, rc::Rc};

use actix_web::{
    body::{AnyBody, MessageBody},
    cookie::{Cookie, CookieJar, Key, SameSite},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::{header::SET_COOKIE, HeaderValue},
    Error, ResponseError,
};
use derive_more::Display;
use futures_util::future::{ok, FutureExt as _, LocalBoxFuture, Ready};
use serde_json::error::Error as JsonError;
use time::{Duration, OffsetDateTime};

use crate::{Session, SessionStatus};

/// Errors that can occur during handling cookie session
#[derive(Debug, Display)]
pub enum CookieSessionError {
    /// Size of the serialized session is greater than 4000 bytes.
    #[display(fmt = "Size of the serialized session is greater than 4000 bytes.")]
    Overflow,

    /// Fail to serialize session.
    #[display(fmt = "Fail to serialize session")]
    Serialize(JsonError),
}

impl ResponseError for CookieSessionError {}

enum CookieSecurity {
    Signed,
    Private,
}

struct CookieSessionInner {
    key: Key,
    security: CookieSecurity,
    name: String,
    path: String,
    domain: Option<String>,
    lazy: bool,
    secure: bool,
    http_only: bool,
    max_age: Option<Duration>,
    expires_in: Option<Duration>,
    same_site: Option<SameSite>,
}

impl CookieSessionInner {
    fn new(key: &[u8], security: CookieSecurity) -> CookieSessionInner {
        CookieSessionInner {
            security,
            key: Key::derive_from(key),
            name: "actix-session".to_owned(),
            path: "/".to_owned(),
            domain: None,
            lazy: false,
            secure: true,
            http_only: true,
            max_age: None,
            expires_in: None,
            same_site: None,
        }
    }

    fn set_cookie<B>(
        &self,
        res: &mut ServiceResponse<B>,
        state: impl Iterator<Item = (String, String)>,
    ) -> Result<(), Error> {
        let state: HashMap<String, String> = state.collect();

        if self.lazy && state.is_empty() {
            return Ok(());
        }

        let value = serde_json::to_string(&state).map_err(CookieSessionError::Serialize)?;

        if value.len() > 4064 {
            return Err(CookieSessionError::Overflow.into());
        }

        let mut cookie = Cookie::new(self.name.clone(), value);
        cookie.set_path(self.path.clone());
        cookie.set_secure(self.secure);
        cookie.set_http_only(self.http_only);

        if let Some(ref domain) = self.domain {
            cookie.set_domain(domain.clone());
        }

        if let Some(expires_in) = self.expires_in {
            cookie.set_expires(OffsetDateTime::now_utc() + expires_in);
        }

        if let Some(max_age) = self.max_age {
            cookie.set_max_age(max_age);
        }

        if let Some(same_site) = self.same_site {
            cookie.set_same_site(same_site);
        }

        let mut jar = CookieJar::new();

        match self.security {
            CookieSecurity::Signed => jar.signed_mut(&self.key).add(cookie),
            CookieSecurity::Private => jar.private_mut(&self.key).add(cookie),
        }

        for cookie in jar.delta() {
            let val = HeaderValue::from_str(&cookie.encoded().to_string())?;
            res.headers_mut().append(SET_COOKIE, val);
        }

        Ok(())
    }

    /// invalidates session cookie
    fn remove_cookie<B>(&self, res: &mut ServiceResponse<B>) -> Result<(), Error> {
        let mut cookie = Cookie::named(self.name.clone());
        cookie.set_path(self.path.clone());
        cookie.set_value("");
        cookie.set_max_age(Duration::zero());
        cookie.set_expires(OffsetDateTime::now_utc() - Duration::days(365));

        let val = HeaderValue::from_str(&cookie.to_string())?;
        res.headers_mut().append(SET_COOKIE, val);

        Ok(())
    }

    fn load(&self, req: &ServiceRequest) -> (bool, HashMap<String, String>) {
        if let Ok(cookies) = req.cookies() {
            for cookie in cookies.iter() {
                if cookie.name() == self.name {
                    let mut jar = CookieJar::new();
                    jar.add_original(cookie.clone());

                    let cookie_opt = match self.security {
                        CookieSecurity::Signed => jar.signed(&self.key).get(&self.name),
                        CookieSecurity::Private => jar.private(&self.key).get(&self.name),
                    };

                    if let Some(cookie) = cookie_opt {
                        if let Ok(val) = serde_json::from_str(cookie.value()) {
                            return (false, val);
                        }
                    }
                }
            }
        }

        (true, HashMap::new())
    }
}

/// Use cookies for session storage.
///
/// `CookieSession` creates sessions which are limited to storing
/// fewer than 4000 bytes of data (as the payload must fit into a single
/// cookie). An Internal Server Error is generated if the session contains more
/// than 4000 bytes.
///
/// A cookie may have a security policy of *signed* or *private*. Each has a
/// respective `CookieSession` constructor.
///
/// A *signed* cookie is stored on the client as plaintext alongside
/// a signature such that the cookie may be viewed but not modified by the
/// client.
///
/// A *private* cookie is stored on the client as encrypted text
/// such that it may neither be viewed nor modified by the client.
///
/// The constructors take a key as an argument.
/// This is the private key for cookie session - when this value is changed,
/// all session data is lost. The constructors will panic if the key is less
/// than 32 bytes in length.
///
/// The backend relies on `cookie` crate to create and read cookies.
/// By default all cookies are percent encoded, but certain symbols may
/// cause troubles when reading cookie, if they are not properly percent encoded.
///
/// # Examples
/// ```
/// use actix_session::CookieSession;
/// use actix_web::{web, App, HttpResponse, HttpServer};
///
/// let app = App::new().wrap(
///     CookieSession::signed(&[0; 32])
///         .domain("www.rust-lang.org")
///         .name("actix_session")
///         .path("/")
///         .secure(true))
///     .service(web::resource("/").to(|| HttpResponse::Ok()));
/// ```
pub struct CookieSession(Rc<CookieSessionInner>);

impl CookieSession {
    /// Construct new *signed* `CookieSession` instance.
    ///
    /// Panics if key length is less than 32 bytes.
    pub fn signed(key: &[u8]) -> CookieSession {
        CookieSession(Rc::new(CookieSessionInner::new(
            key,
            CookieSecurity::Signed,
        )))
    }

    /// Construct new *private* `CookieSession` instance.
    ///
    /// Panics if key length is less than 32 bytes.
    pub fn private(key: &[u8]) -> CookieSession {
        CookieSession(Rc::new(CookieSessionInner::new(
            key,
            CookieSecurity::Private,
        )))
    }

    /// Sets the `path` field in the session cookie being built.
    pub fn path<S: Into<String>>(mut self, value: S) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().path = value.into();
        self
    }

    /// Sets the `name` field in the session cookie being built.
    pub fn name<S: Into<String>>(mut self, value: S) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().name = value.into();
        self
    }

    /// Sets the `domain` field in the session cookie being built.
    pub fn domain<S: Into<String>>(mut self, value: S) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().domain = Some(value.into());
        self
    }

    /// When true, prevents adding session cookies to responses until
    /// the session contains data. Default is `false`.
    ///
    /// Useful when trying to comply with laws that require consent for setting cookies.
    pub fn lazy(mut self, value: bool) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().lazy = value;
        self
    }

    /// Sets the `secure` field in the session cookie being built.
    ///
    /// If the `secure` field is set, a cookie will only be transmitted when the
    /// connection is secure - i.e. `https`
    pub fn secure(mut self, value: bool) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().secure = value;
        self
    }

    /// Sets the `http_only` field in the session cookie being built.
    pub fn http_only(mut self, value: bool) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().http_only = value;
        self
    }

    /// Sets the `same_site` field in the session cookie being built.
    pub fn same_site(mut self, value: SameSite) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().same_site = Some(value);
        self
    }

    /// Sets the `max-age` field in the session cookie being built.
    pub fn max_age(self, seconds: i64) -> CookieSession {
        self.max_age_time(Duration::seconds(seconds))
    }

    /// Sets the `max-age` field in the session cookie being built.
    pub fn max_age_time(mut self, value: time::Duration) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().max_age = Some(value);
        self
    }

    /// Sets the `expires` field in the session cookie being built.
    pub fn expires_in(self, seconds: i64) -> CookieSession {
        self.expires_in_time(Duration::seconds(seconds))
    }

    /// Sets the `expires` field in the session cookie being built.
    pub fn expires_in_time(mut self, value: Duration) -> CookieSession {
        Rc::get_mut(&mut self.0).unwrap().expires_in = Some(value);
        self
    }
}

impl<S, B> Transform<S, ServiceRequest> for CookieSession
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>>,
    S::Future: 'static,
    S::Error: 'static,
    B: MessageBody + 'static,
    B::Error: StdError,
{
    type Response = ServiceResponse;
    type Error = S::Error;
    type InitError = ();
    type Transform = CookieSessionMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CookieSessionMiddleware {
            service,
            inner: self.0.clone(),
        })
    }
}

/// Cookie based session middleware.
pub struct CookieSessionMiddleware<S> {
    service: S,
    inner: Rc<CookieSessionInner>,
}

impl<S, B> Service<ServiceRequest> for CookieSessionMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>>,
    S::Future: 'static,
    S::Error: 'static,
    B: MessageBody + 'static,
    B::Error: StdError,
{
    type Response = ServiceResponse;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_service::forward_ready!(service);

    /// On first request, a new session cookie is returned in response, regardless
    /// of whether any session state is set.  With subsequent requests, if the
    /// session state changes, then set-cookie is returned in response.  As
    /// a user logs out, call session.purge() to set SessionStatus accordingly
    /// and this will trigger removal of the session cookie in the response.
    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let inner = self.inner.clone();
        let (is_new, state) = self.inner.load(&req);
        let prolong_expiration = self.inner.expires_in.is_some();
        Session::set_session(&mut req, state);

        let fut = self.service.call(req);

        async move {
            let mut res = fut.await?;

            let result = match Session::get_changes(&mut res) {
                (SessionStatus::Changed, state) | (SessionStatus::Renewed, state) => {
                    inner.set_cookie(&mut res, state)
                }

                (SessionStatus::Unchanged, state) if prolong_expiration => {
                    inner.set_cookie(&mut res, state)
                }

                // set a new session cookie upon first request (new client)
                (SessionStatus::Unchanged, _) => {
                    if is_new {
                        let state: HashMap<String, String> = HashMap::new();
                        inner.set_cookie(&mut res, state.into_iter())
                    } else {
                        Ok(())
                    }
                }

                (SessionStatus::Purged, _) => {
                    let _ = inner.remove_cookie(&mut res);
                    Ok(())
                }
            };

            match result {
                Ok(()) => Ok(res.map_body(|_, body| AnyBody::from_message(body))),
                Err(error) => Ok(res.error_response(error)),
            }
        }
        .boxed_local()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::web::Bytes;
    use actix_web::{test, web, App};

    #[actix_rt::test]
    async fn cookie_session() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn private_cookie() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::private(&[0; 32]).secure(false))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn lazy_cookie() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false).lazy(true))
                .service(web::resource("/count").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "counting"
                }))
                .service(web::resource("/").to(|_ses: Session| async move { "test" })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response.response().cookies().count() == 0);

        let request = test::TestRequest::with_uri("/count").to_request();
        let response = app.call(request).await.unwrap();

        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn cookie_session_extractor() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn basics() {
        let app = test::init_service(
            App::new()
                .wrap(
                    CookieSession::signed(&[0; 32])
                        .path("/test/")
                        .name("actix-test")
                        .domain("localhost")
                        .http_only(true)
                        .same_site(SameSite::Lax)
                        .max_age(100),
                )
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                }))
                .service(web::resource("/test/").to(|ses: Session| async move {
                    let val: usize = ses.get("counter").unwrap().unwrap();
                    format!("counter: {}", val)
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        let cookie = response
            .response()
            .cookies()
            .find(|c| c.name() == "actix-test")
            .unwrap()
            .clone();
        assert_eq!(cookie.path().unwrap(), "/test/");

        let request = test::TestRequest::with_uri("/test/")
            .cookie(cookie)
            .to_request();
        let body = test::read_response(&app, request).await;
        assert_eq!(body, Bytes::from_static(b"counter: 100"));
    }

    #[actix_rt::test]
    async fn prolong_expiration() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false).expires_in(60))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                }))
                .service(web::resource("/test/").to(|| async move { "no-changes-in-session" })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        let expires_1 = response
            .response()
            .cookies()
            .find(|c| c.name() == "actix-session")
            .expect("Cookie is set")
            .expires()
            .expect("Expiration is set")
            .datetime()
            .expect("Expiration is a datetime");

        actix_rt::time::sleep(std::time::Duration::from_secs(1)).await;

        let request = test::TestRequest::with_uri("/test/").to_request();
        let response = app.call(request).await.unwrap();
        let expires_2 = response
            .response()
            .cookies()
            .find(|c| c.name() == "actix-session")
            .expect("Cookie is set")
            .expires()
            .expect("Expiration is set")
            .datetime()
            .expect("Expiration is a datetime");

        assert!(expires_2 - expires_1 >= Duration::seconds(1));
    }
}
