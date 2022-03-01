use std::{rc::Rc, time::SystemTime};

use actix_utils::future::{ready, Ready};
use serde::{Deserialize, Serialize};
use time::Duration;

use actix_web::{
    cookie::{Cookie, CookieJar, Key, SameSite},
    dev::{ServiceRequest, ServiceResponse},
    error::{Error, Result},
    http::header::{self, HeaderValue},
    HttpMessage,
};

use crate::IdentityPolicy;

struct CookieIdentityInner {
    key: Key,
    key_v2: Key,
    name: String,
    path: String,
    domain: Option<String>,
    secure: bool,
    max_age: Option<Duration>,
    http_only: Option<bool>,
    same_site: Option<SameSite>,
    visit_deadline: Option<Duration>,
    login_deadline: Option<Duration>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CookieValue {
    identity: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    login_timestamp: Option<SystemTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    visit_timestamp: Option<SystemTime>,
}

#[derive(Debug)]
struct CookieIdentityExtension {
    login_timestamp: Option<SystemTime>,
}

impl CookieIdentityInner {
    fn new(key: &[u8]) -> CookieIdentityInner {
        let key_v2: Vec<u8> = [key, &[1, 0, 0, 0]].concat();

        CookieIdentityInner {
            key: Key::derive_from(key),
            key_v2: Key::derive_from(&key_v2),
            name: "actix-identity".to_owned(),
            path: "/".to_owned(),
            domain: None,
            secure: true,
            max_age: None,
            http_only: None,
            same_site: None,
            visit_deadline: None,
            login_deadline: None,
        }
    }

    fn set_cookie<B>(
        &self,
        resp: &mut ServiceResponse<B>,
        value: Option<CookieValue>,
    ) -> Result<()> {
        let add_cookie = value.is_some();
        let val = value
            .map(|val| {
                if !self.legacy_supported() {
                    serde_json::to_string(&val)
                } else {
                    Ok(val.identity)
                }
            })
            .transpose()?;

        let mut cookie = Cookie::new(self.name.clone(), val.unwrap_or_default());
        cookie.set_path(self.path.clone());
        cookie.set_secure(self.secure);
        cookie.set_http_only(true);

        if let Some(ref domain) = self.domain {
            cookie.set_domain(domain.clone());
        }

        if let Some(max_age) = self.max_age {
            cookie.set_max_age(max_age);
        }

        if let Some(http_only) = self.http_only {
            cookie.set_http_only(http_only);
        }

        if let Some(same_site) = self.same_site {
            cookie.set_same_site(same_site);
        }

        let mut jar = CookieJar::new();

        let key = if self.legacy_supported() {
            &self.key
        } else {
            &self.key_v2
        };

        if add_cookie {
            jar.private_mut(key).add(cookie);
        } else {
            jar.add_original(cookie.clone());
            jar.private_mut(key).remove(cookie);
        }

        for cookie in jar.delta() {
            let val = HeaderValue::from_str(&cookie.to_string())?;
            resp.headers_mut().append(header::SET_COOKIE, val);
        }

        Ok(())
    }

    fn load(&self, req: &ServiceRequest) -> Option<CookieValue> {
        let cookie = req.cookie(&self.name)?;
        let mut jar = CookieJar::new();
        jar.add_original(cookie.clone());

        let res = if self.legacy_supported() {
            jar.private_mut(&self.key)
                .get(&self.name)
                .map(|n| CookieValue {
                    identity: n.value().to_string(),
                    login_timestamp: None,
                    visit_timestamp: None,
                })
        } else {
            None
        };

        res.or_else(|| {
            jar.private_mut(&self.key_v2)
                .get(&self.name)
                .and_then(|c| self.parse(c))
        })
    }

    fn parse(&self, cookie: Cookie<'_>) -> Option<CookieValue> {
        let value: CookieValue = serde_json::from_str(cookie.value()).ok()?;
        let now = SystemTime::now();

        if let Some(visit_deadline) = self.visit_deadline {
            let inactivity = now.duration_since(value.visit_timestamp?).ok()?;

            if inactivity > visit_deadline {
                return None;
            }
        }

        if let Some(login_deadline) = self.login_deadline {
            let logged_in_dur = now.duration_since(value.login_timestamp?).ok()?;

            if logged_in_dur > login_deadline {
                return None;
            }
        }

        Some(value)
    }

    fn legacy_supported(&self) -> bool {
        self.visit_deadline.is_none() && self.login_deadline.is_none()
    }

    fn always_update_cookie(&self) -> bool {
        self.visit_deadline.is_some()
    }

    fn requires_oob_data(&self) -> bool {
        self.login_deadline.is_some()
    }
}

/// Use cookies for request identity storage.
///
/// [See this page on MDN](mdn-cookies) for details on cookie attributes.
///
/// # Examples
/// ```
/// use actix_web::App;
/// use actix_identity::{CookieIdentityPolicy, IdentityService};
///
/// // create cookie identity backend
/// let policy = CookieIdentityPolicy::new(&[0; 32])
///            .domain("www.rust-lang.org")
///            .name("actix_auth")
///            .path("/")
///            .secure(true);
///
/// let app = App::new()
///     // wrap policy into identity middleware
///     .wrap(IdentityService::new(policy));
/// ```
///
/// [mdn-cookies]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Cookies
pub struct CookieIdentityPolicy(Rc<CookieIdentityInner>);

impl CookieIdentityPolicy {
    /// Create new `CookieIdentityPolicy` instance.
    ///
    /// Key argument is the private key for issued cookies. If this value is changed, all issued
    /// cookie identities are invalidated.
    ///
    /// # Panics
    /// Panics if `key` is less than 32 bytes in length..
    pub fn new(key: &[u8]) -> CookieIdentityPolicy {
        CookieIdentityPolicy(Rc::new(CookieIdentityInner::new(key)))
    }

    /// Sets the name of issued cookies.
    pub fn name(mut self, value: impl Into<String>) -> CookieIdentityPolicy {
        self.inner_mut().name = value.into();
        self
    }

    /// Sets the `Path` attribute of issued cookies.
    pub fn path(mut self, value: impl Into<String>) -> CookieIdentityPolicy {
        self.inner_mut().path = value.into();
        self
    }

    /// Sets the `Domain` attribute of issued cookies.
    pub fn domain(mut self, value: impl Into<String>) -> CookieIdentityPolicy {
        self.inner_mut().domain = Some(value.into());
        self
    }

    /// Sets the `Secure` attribute of issued cookies.
    pub fn secure(mut self, value: bool) -> CookieIdentityPolicy {
        self.inner_mut().secure = value;
        self
    }

    /// Sets the `Max-Age` attribute of issued cookies.
    pub fn max_age(mut self, value: Duration) -> CookieIdentityPolicy {
        self.inner_mut().max_age = Some(value);
        self
    }

    /// Sets the `Max-Age` attribute of issued cookies with given number of seconds.
    pub fn max_age_secs(self, seconds: i64) -> CookieIdentityPolicy {
        self.max_age(Duration::seconds(seconds))
    }

    /// Sets the `HttpOnly` attribute of issued cookies.
    ///
    /// By default, the `HttpOnly` attribute is omitted from issued cookies.
    pub fn http_only(mut self, http_only: bool) -> Self {
        self.inner_mut().http_only = Some(http_only);
        self
    }

    /// Sets the `SameSite` attribute of issued cookies.
    ///
    /// By default, the `SameSite` attribute is omitted from issued cookies.
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.inner_mut().same_site = Some(same_site);
        self
    }

    /// Accepts only users who have visited within given deadline.
    ///
    /// In other words, invalidate a login after some amount of inactivity. Using this feature
    /// causes updated cookies to be issued on each response in order to record the user's last
    /// visitation timestamp.
    ///
    /// By default, visit deadline is disabled.
    pub fn visit_deadline(mut self, deadline: Duration) -> CookieIdentityPolicy {
        self.inner_mut().visit_deadline = Some(deadline);
        self
    }

    /// Accepts only users who authenticated within the given deadline.
    ///
    /// In other words, invalidate a login after some amount of time, regardless of activity.
    /// While [`Max-Age`](CookieIdentityPolicy::max_age) is useful in constraining the cookie
    /// lifetime, it could be extended manually; using this feature encodes the deadline directly
    /// into the issued cookies, making it immutable to users.
    ///
    /// By default, login deadline is disabled.
    pub fn login_deadline(mut self, deadline: Duration) -> CookieIdentityPolicy {
        self.inner_mut().login_deadline = Some(deadline);
        self
    }

    fn inner_mut(&mut self) -> &mut CookieIdentityInner {
        Rc::get_mut(&mut self.0).unwrap()
    }
}

impl IdentityPolicy for CookieIdentityPolicy {
    type Future = Ready<Result<Option<String>, Error>>;
    type ResponseFuture = Ready<Result<(), Error>>;

    fn from_request(&self, req: &mut ServiceRequest) -> Self::Future {
        ready(Ok(self.0.load(req).map(|value| {
            let CookieValue {
                identity,
                login_timestamp,
                ..
            } = value;

            if self.0.requires_oob_data() {
                req.extensions_mut()
                    .insert(CookieIdentityExtension { login_timestamp });
            }

            identity
        })))
    }

    fn to_response<B>(
        &self,
        id: Option<String>,
        changed: bool,
        res: &mut ServiceResponse<B>,
    ) -> Self::ResponseFuture {
        let _ = if changed {
            let login_timestamp = SystemTime::now();

            self.0.set_cookie(
                res,
                id.map(|identity| CookieValue {
                    identity,
                    login_timestamp: self.0.login_deadline.map(|_| login_timestamp),
                    visit_timestamp: self.0.visit_deadline.map(|_| login_timestamp),
                }),
            )
        } else if self.0.always_update_cookie() && id.is_some() {
            let visit_timestamp = SystemTime::now();

            let login_timestamp = if self.0.requires_oob_data() {
                let CookieIdentityExtension { login_timestamp } =
                    res.request().extensions_mut().remove().unwrap();

                login_timestamp
            } else {
                None
            };

            self.0.set_cookie(
                res,
                Some(CookieValue {
                    identity: id.unwrap(),
                    login_timestamp,
                    visit_timestamp: self.0.visit_deadline.map(|_| visit_timestamp),
                }),
            )
        } else {
            Ok(())
        };

        ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Borrow, time::SystemTime};

    use actix_web::{
        body::{BoxBody, EitherBody},
        cookie::{Cookie, CookieJar, Key, SameSite},
        dev::ServiceResponse,
        http::{header, StatusCode},
        test::{self, TestRequest},
        web, App, HttpResponse,
    };
    use time::Duration;

    use super::*;
    use crate::{tests::*, Identity, IdentityService};

    fn login_cookie(
        identity: &'static str,
        login_timestamp: Option<SystemTime>,
        visit_timestamp: Option<SystemTime>,
    ) -> Cookie<'static> {
        let mut jar = CookieJar::new();
        let key: Vec<u8> = COOKIE_KEY_MASTER
            .iter()
            .chain([1, 0, 0, 0].iter())
            .copied()
            .collect();

        jar.private_mut(&Key::derive_from(&key)).add(Cookie::new(
            COOKIE_NAME,
            serde_json::to_string(&CookieValue {
                identity: identity.to_string(),
                login_timestamp,
                visit_timestamp,
            })
            .unwrap(),
        ));

        jar.get(COOKIE_NAME).unwrap().clone()
    }

    fn assert_login_cookie(
        response: &mut ServiceResponse<EitherBody<BoxBody>>,
        identity: &str,
        login_timestamp: LoginTimestampCheck,
        visit_timestamp: VisitTimeStampCheck,
    ) {
        let mut cookies = CookieJar::new();

        for cookie in response.headers().get_all(header::SET_COOKIE) {
            cookies.add(Cookie::parse(cookie.to_str().unwrap().to_string()).unwrap());
        }

        let key: Vec<u8> = COOKIE_KEY_MASTER
            .iter()
            .chain([1, 0, 0, 0].iter())
            .copied()
            .collect();

        let cookie = cookies
            .private(&Key::derive_from(&key))
            .get(COOKIE_NAME)
            .unwrap();

        let cv: CookieValue = serde_json::from_str(cookie.value()).unwrap();
        assert_eq!(cv.identity, identity);

        let now = SystemTime::now();
        let t30sec_ago = now - Duration::seconds(30);

        match login_timestamp {
            LoginTimestampCheck::NoTimestamp => assert_eq!(cv.login_timestamp, None),
            LoginTimestampCheck::NewTimestamp => assert!(
                t30sec_ago <= cv.login_timestamp.unwrap() && cv.login_timestamp.unwrap() <= now
            ),
            LoginTimestampCheck::OldTimestamp(old_timestamp) => {
                assert_eq!(cv.login_timestamp, Some(old_timestamp))
            }
        }

        match visit_timestamp {
            VisitTimeStampCheck::NoTimestamp => assert_eq!(cv.visit_timestamp, None),
            VisitTimeStampCheck::NewTimestamp => assert!(
                t30sec_ago <= cv.visit_timestamp.unwrap() && cv.visit_timestamp.unwrap() <= now
            ),
        }
    }

    #[actix_web::test]
    async fn test_identity_flow() {
        let srv = test::init_service(
            App::new()
                .wrap(IdentityService::new(
                    CookieIdentityPolicy::new(&COOKIE_KEY_MASTER)
                        .domain("www.rust-lang.org")
                        .name(COOKIE_NAME)
                        .path("/")
                        .secure(true),
                ))
                .service(web::resource("/index").to(|id: Identity| {
                    if id.identity().is_some() {
                        HttpResponse::Created()
                    } else {
                        HttpResponse::Ok()
                    }
                }))
                .service(web::resource("/login").to(|id: Identity| {
                    id.remember(COOKIE_LOGIN.to_string());
                    HttpResponse::Ok()
                }))
                .service(web::resource("/logout").to(|id: Identity| {
                    if id.identity().is_some() {
                        id.forget();
                        HttpResponse::Ok()
                    } else {
                        HttpResponse::BadRequest()
                    }
                })),
        )
        .await;
        let resp = test::call_service(&srv, TestRequest::with_uri("/index").to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = test::call_service(&srv, TestRequest::with_uri("/login").to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let c = resp.response().cookies().next().unwrap().to_owned();

        let resp = test::call_service(
            &srv,
            TestRequest::with_uri("/index")
                .cookie(c.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = test::call_service(
            &srv,
            TestRequest::with_uri("/logout")
                .cookie(c.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key(header::SET_COOKIE))
    }

    #[actix_web::test]
    async fn test_identity_max_age_time() {
        let duration = Duration::days(1);

        let srv = test::init_service(
            App::new()
                .wrap(IdentityService::new(
                    CookieIdentityPolicy::new(&COOKIE_KEY_MASTER)
                        .domain("www.rust-lang.org")
                        .name(COOKIE_NAME)
                        .path("/")
                        .max_age(duration)
                        .secure(true),
                ))
                .service(web::resource("/login").to(|id: Identity| {
                    id.remember("test".to_string());
                    HttpResponse::Ok()
                })),
        )
        .await;

        let resp = test::call_service(&srv, TestRequest::with_uri("/login").to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key(header::SET_COOKIE));
        let c = resp.response().cookies().next().unwrap().to_owned();
        assert_eq!(duration, c.max_age().unwrap());
    }

    #[actix_web::test]
    async fn test_http_only_same_site() {
        let srv = test::init_service(
            App::new()
                .wrap(IdentityService::new(
                    CookieIdentityPolicy::new(&COOKIE_KEY_MASTER)
                        .domain("www.rust-lang.org")
                        .name(COOKIE_NAME)
                        .path("/")
                        .http_only(true)
                        .same_site(SameSite::None),
                ))
                .service(web::resource("/login").to(|id: Identity| {
                    id.remember("test".to_string());
                    HttpResponse::Ok()
                })),
        )
        .await;

        let resp = test::call_service(&srv, TestRequest::with_uri("/login").to_request()).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key(header::SET_COOKIE));

        let c = resp.response().cookies().next().unwrap().to_owned();
        assert!(c.http_only().unwrap());
        assert_eq!(SameSite::None, c.same_site().unwrap());
    }

    fn legacy_login_cookie(identity: &'static str) -> Cookie<'static> {
        let mut jar = CookieJar::new();
        jar.private_mut(&Key::derive_from(&COOKIE_KEY_MASTER))
            .add(Cookie::new(COOKIE_NAME, identity));
        jar.get(COOKIE_NAME).unwrap().clone()
    }

    async fn assert_logged_in(
        response: ServiceResponse<EitherBody<BoxBody>>,
        identity: Option<&str>,
    ) {
        let bytes = test::read_body(response).await;
        let resp: Option<String> = serde_json::from_slice(&bytes[..]).unwrap();
        assert_eq!(resp.as_ref().map(|s| s.borrow()), identity);
    }

    fn assert_legacy_login_cookie(
        response: &mut ServiceResponse<EitherBody<BoxBody>>,
        identity: &str,
    ) {
        let mut cookies = CookieJar::new();
        for cookie in response.headers().get_all(header::SET_COOKIE) {
            cookies.add(Cookie::parse(cookie.to_str().unwrap().to_string()).unwrap());
        }
        let cookie = cookies
            .private_mut(&Key::derive_from(&COOKIE_KEY_MASTER))
            .get(COOKIE_NAME)
            .unwrap();
        assert_eq!(cookie.value(), identity);
    }

    fn assert_no_login_cookie(response: &mut ServiceResponse<EitherBody<BoxBody>>) {
        let mut cookies = CookieJar::new();
        for cookie in response.headers().get_all(header::SET_COOKIE) {
            cookies.add(Cookie::parse(cookie.to_str().unwrap().to_string()).unwrap());
        }
        assert!(cookies.get(COOKIE_NAME).is_none());
    }

    #[actix_web::test]
    async fn test_identity_max_age() {
        let seconds = 60;
        let srv = test::init_service(
            App::new()
                .wrap(IdentityService::new(
                    CookieIdentityPolicy::new(&COOKIE_KEY_MASTER)
                        .domain("www.rust-lang.org")
                        .name(COOKIE_NAME)
                        .path("/")
                        .max_age_secs(seconds)
                        .secure(true),
                ))
                .service(web::resource("/login").to(|id: Identity| {
                    id.remember("test".to_string());
                    HttpResponse::Ok()
                })),
        )
        .await;
        let resp = test::call_service(&srv, TestRequest::with_uri("/login").to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key(header::SET_COOKIE));
        let c = resp.response().cookies().next().unwrap().to_owned();
        assert_eq!(Duration::seconds(seconds as i64), c.max_age().unwrap());
    }

    #[actix_web::test]
    async fn test_identity_legacy_cookie_is_set() {
        let srv = create_identity_server(|c| c).await;
        let mut resp = test::call_service(&srv, TestRequest::with_uri("/").to_request()).await;
        assert_legacy_login_cookie(&mut resp, COOKIE_LOGIN);
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_legacy_cookie_works() {
        let srv = create_identity_server(|c| c).await;
        let cookie = legacy_login_cookie(COOKIE_LOGIN);
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_no_login_cookie(&mut resp);
        assert_logged_in(resp, Some(COOKIE_LOGIN)).await;
    }

    #[actix_web::test]
    async fn test_identity_legacy_cookie_rejected_if_visit_timestamp_needed() {
        let srv = create_identity_server(|c| c.visit_deadline(Duration::days(90))).await;
        let cookie = legacy_login_cookie(COOKIE_LOGIN);
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::NoTimestamp,
            VisitTimeStampCheck::NewTimestamp,
        );
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_legacy_cookie_rejected_if_login_timestamp_needed() {
        let srv = create_identity_server(|c| c.login_deadline(Duration::days(90))).await;
        let cookie = legacy_login_cookie(COOKIE_LOGIN);
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::NewTimestamp,
            VisitTimeStampCheck::NoTimestamp,
        );
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_cookie_rejected_if_login_timestamp_needed() {
        let srv = create_identity_server(|c| c.login_deadline(Duration::days(90))).await;
        let cookie = login_cookie(COOKIE_LOGIN, None, Some(SystemTime::now()));
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::NewTimestamp,
            VisitTimeStampCheck::NoTimestamp,
        );
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_cookie_rejected_if_visit_timestamp_needed() {
        let srv = create_identity_server(|c| c.visit_deadline(Duration::days(90))).await;
        let cookie = login_cookie(COOKIE_LOGIN, Some(SystemTime::now()), None);
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::NoTimestamp,
            VisitTimeStampCheck::NewTimestamp,
        );
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_cookie_rejected_if_login_timestamp_too_old() {
        let srv = create_identity_server(|c| c.login_deadline(Duration::days(90))).await;
        let cookie = login_cookie(
            COOKIE_LOGIN,
            Some(SystemTime::now() - Duration::days(180)),
            None,
        );
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::NewTimestamp,
            VisitTimeStampCheck::NoTimestamp,
        );
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_cookie_rejected_if_visit_timestamp_too_old() {
        let srv = create_identity_server(|c| c.visit_deadline(Duration::days(90))).await;
        let cookie = login_cookie(
            COOKIE_LOGIN,
            None,
            Some(SystemTime::now() - Duration::days(180)),
        );
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::NoTimestamp,
            VisitTimeStampCheck::NewTimestamp,
        );
        assert_logged_in(resp, None).await;
    }

    #[actix_web::test]
    async fn test_identity_cookie_not_updated_on_login_deadline() {
        let srv = create_identity_server(|c| c.login_deadline(Duration::days(90))).await;
        let cookie = login_cookie(COOKIE_LOGIN, Some(SystemTime::now()), None);
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_no_login_cookie(&mut resp);
        assert_logged_in(resp, Some(COOKIE_LOGIN)).await;
    }

    #[actix_web::test]
    async fn test_identity_cookie_updated_on_visit_deadline() {
        let srv = create_identity_server(|c| {
            c.visit_deadline(Duration::days(90))
                .login_deadline(Duration::days(90))
        })
        .await;
        let timestamp = SystemTime::now() - Duration::days(1);
        let cookie = login_cookie(COOKIE_LOGIN, Some(timestamp), Some(timestamp));
        let mut resp = test::call_service(
            &srv,
            TestRequest::with_uri("/")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_login_cookie(
            &mut resp,
            COOKIE_LOGIN,
            LoginTimestampCheck::OldTimestamp(timestamp),
            VisitTimeStampCheck::NewTimestamp,
        );
        assert_logged_in(resp, Some(COOKIE_LOGIN)).await;
    }
}
