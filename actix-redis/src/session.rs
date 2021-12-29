use std::{collections::HashMap, iter, rc::Rc};

use actix::prelude::*;
use actix_service::{Service, Transform};
use actix_session::{Session, SessionStatus};
use actix_web::{
    cookie::{Cookie, CookieJar, Key, SameSite},
    dev::{ServiceRequest, ServiceResponse},
    error,
    http::header::{self, HeaderValue},
    Error,
};
use futures_core::future::LocalBoxFuture;
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use redis_async::{resp::RespValue, resp_array};
use time::{self, Duration, OffsetDateTime};

use crate::redis::{Command, RedisActor};

/// Use redis as session storage.
///
/// You need to pass an address of the redis server and random value to the
/// constructor of `RedisSession`. This is private key for cookie
/// session, When this value is changed, all session data is lost.
///
/// Constructor panics if key length is less than 32 bytes.
pub struct RedisSession(Rc<Inner>);

impl RedisSession {
    /// Create new redis session backend
    ///
    /// * `addr` - address of the redis server
    pub fn new<S: Into<String>>(addr: S, key: &[u8]) -> RedisSession {
        RedisSession(Rc::new(Inner {
            key: Key::derive_from(key),
            cache_keygen: Box::new(|key: &str| format!("session:{}", &key)),
            ttl: "7200".to_owned(),
            addr: RedisActor::start(addr),
            name: "actix-session".to_owned(),
            path: "/".to_owned(),
            domain: None,
            secure: false,
            max_age: Some(Duration::days(7)),
            same_site: None,
            http_only: true,
        }))
    }

    /// Set time to live in seconds for session value.
    pub fn ttl(mut self, ttl: u32) -> Self {
        Rc::get_mut(&mut self.0).unwrap().ttl = format!("{}", ttl);
        self
    }

    /// Set custom cookie name for session ID.
    pub fn cookie_name(mut self, name: &str) -> Self {
        Rc::get_mut(&mut self.0).unwrap().name = name.to_owned();
        self
    }

    /// Set custom cookie path.
    pub fn cookie_path(mut self, path: &str) -> Self {
        Rc::get_mut(&mut self.0).unwrap().path = path.to_owned();
        self
    }

    /// Set custom cookie domain.
    pub fn cookie_domain(mut self, domain: &str) -> Self {
        Rc::get_mut(&mut self.0).unwrap().domain = Some(domain.to_owned());
        self
    }

    /// Set custom cookie secure.
    ///
    /// If the `secure` field is set, a cookie will only be transmitted when the
    /// connection is secure - i.e. `https`.
    ///
    /// Default is false.
    pub fn cookie_secure(mut self, secure: bool) -> Self {
        Rc::get_mut(&mut self.0).unwrap().secure = secure;
        self
    }

    /// Set custom cookie max-age.
    ///
    /// Use `None` for session-only cookies.
    pub fn cookie_max_age(mut self, max_age: impl Into<Option<Duration>>) -> Self {
        Rc::get_mut(&mut self.0).unwrap().max_age = max_age.into();
        self
    }

    /// Set custom cookie `SameSite` attribute.
    ///
    /// By default, the attribute is omitted.
    pub fn cookie_same_site(mut self, same_site: SameSite) -> Self {
        Rc::get_mut(&mut self.0).unwrap().same_site = Some(same_site);
        self
    }

    /// Set custom cookie `HttpOnly` policy.
    ///
    /// Default is true.
    pub fn cookie_http_only(mut self, http_only: bool) -> Self {
        Rc::get_mut(&mut self.0).unwrap().http_only = http_only;
        self
    }

    /// Set a custom cache key generation strategy, expecting session key as input.
    pub fn cache_keygen(mut self, keygen: Box<dyn Fn(&str) -> String>) -> Self {
        Rc::get_mut(&mut self.0).unwrap().cache_keygen = keygen;
        self
    }
}

impl<S, B> Transform<S, ServiceRequest> for RedisSession
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Transform = RedisSessionMiddleware<S>;
    type InitError = ();
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let inner = self.0.clone();
        Box::pin(async {
            Ok(RedisSessionMiddleware {
                service: Rc::new(service),
                inner,
            })
        })
    }
}

/// Cookie session middleware
pub struct RedisSessionMiddleware<S: 'static> {
    service: Rc<S>,
    inner: Rc<Inner>,
}

impl<S, B> Service<ServiceRequest> for RedisSessionMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_service::forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let srv = Rc::clone(&self.service);
        let inner = Rc::clone(&self.inner);

        Box::pin(async move {
            let state = inner.load(&req).await?;

            let value = if let Some((state, value)) = state {
                Session::set_session(&mut req, state);
                Some(value)
            } else {
                None
            };

            let mut res = srv.call(req).await?;

            match Session::get_changes(&mut res) {
                (SessionStatus::Unchanged, _) => {
                    // If the session already exists, we don't need to update the state stored in Redis
                    // If the session is new, creating an empty session in Redis is unnecessary overhead
                    Ok(res)
                }

                (SessionStatus::Changed, state) => inner.update(res, state, value).await,

                (SessionStatus::Purged, _) => {
                    if let Some(val) = value {
                        inner.clear_cache(val).await?;
                        match inner.remove_cookie(&mut res) {
                            Ok(_) => Ok(res),
                            Err(_err) => Err(error::ErrorInternalServerError(_err)),
                        }
                    } else {
                        Err(error::ErrorInternalServerError("unexpected"))
                    }
                }

                (SessionStatus::Renewed, state) => {
                    if let Some(val) = value {
                        inner.clear_cache(val).await?;
                        inner.update(res, state, None).await
                    } else {
                        inner.update(res, state, None).await
                    }
                }
            }
        })
    }
}

struct Inner {
    key: Key,
    cache_keygen: Box<dyn Fn(&str) -> String>,
    ttl: String,
    addr: Addr<RedisActor>,
    name: String,
    path: String,
    domain: Option<String>,
    secure: bool,
    max_age: Option<Duration>,
    same_site: Option<SameSite>,
    http_only: bool,
}

impl Inner {
    async fn load(
        &self,
        req: &ServiceRequest,
    ) -> Result<Option<(HashMap<String, String>, String)>, Error> {
        // wrapped in block to avoid holding `Ref` (from `req.cookies`) across await point
        let (value, cache_key) = {
            let cookies = if let Ok(cookies) = req.cookies() {
                cookies
            } else {
                return Ok(None);
            };

            if let Some(cookie) = cookies.iter().find(|&cookie| cookie.name() == self.name) {
                let mut jar = CookieJar::new();
                jar.add_original(cookie.clone());

                if let Some(cookie) = jar.signed(&self.key).get(&self.name) {
                    let value = cookie.value().to_owned();
                    let cache_key = (self.cache_keygen)(cookie.value());
                    (value, cache_key)
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }
        };

        let val = self
            .addr
            .send(Command(resp_array!["GET", cache_key]))
            .await
            .map_err(error::ErrorInternalServerError)?
            .map_err(error::ErrorInternalServerError)?;

        match val {
            RespValue::Error(err) => {
                return Err(error::ErrorInternalServerError(err));
            }
            RespValue::SimpleString(s) => {
                if let Ok(val) = serde_json::from_str(&s) {
                    return Ok(Some((val, value)));
                }
            }
            RespValue::BulkString(s) => {
                if let Ok(val) = serde_json::from_slice(&s) {
                    return Ok(Some((val, value)));
                }
            }
            _ => {}
        }

        Ok(None)
    }

    async fn update<B>(
        &self,
        mut res: ServiceResponse<B>,
        state: impl Iterator<Item = (String, String)>,
        value: Option<String>,
    ) -> Result<ServiceResponse<B>, Error> {
        let (value, jar) = if let Some(value) = value {
            (value, None)
        } else {
            let value = iter::repeat(())
                .map(|()| OsRng.sample(Alphanumeric))
                .take(32)
                .collect::<Vec<_>>();
            let value = String::from_utf8(value).unwrap_or_default();

            // prepare session id cookie
            let mut cookie = Cookie::new(self.name.clone(), value.clone());
            cookie.set_path(self.path.clone());
            cookie.set_secure(self.secure);
            cookie.set_http_only(self.http_only);

            if let Some(ref domain) = self.domain {
                cookie.set_domain(domain.clone());
            }

            if let Some(max_age) = self.max_age {
                cookie.set_max_age(max_age);
            }

            if let Some(same_site) = self.same_site {
                cookie.set_same_site(same_site);
            }

            // set cookie
            let mut jar = CookieJar::new();
            jar.signed_mut(&self.key).add(cookie);

            (value, Some(jar))
        };

        let cache_key = (self.cache_keygen)(&value);

        let state: HashMap<_, _> = state.collect();

        let body = match serde_json::to_string(&state) {
            Err(err) => return Err(err.into()),
            Ok(body) => body,
        };

        let cmd = Command(resp_array!["SET", cache_key, body, "EX", &self.ttl]);

        self.addr
            .send(cmd)
            .await
            .map_err(error::ErrorInternalServerError)?
            .map_err(error::ErrorInternalServerError)?;

        if let Some(jar) = jar {
            for cookie in jar.delta() {
                let val = HeaderValue::from_str(&cookie.to_string())?;
                res.headers_mut().append(header::SET_COOKIE, val);
            }
        }

        Ok(res)
    }

    /// Removes cache entry.
    async fn clear_cache(&self, key: String) -> Result<(), Error> {
        let cache_key = (self.cache_keygen)(&key);

        let res = self
            .addr
            .send(Command(resp_array!["DEL", cache_key]))
            .await
            .map_err(error::ErrorInternalServerError)?;

        match res {
            // redis responds with number of deleted records
            Ok(RespValue::Integer(x)) if x > 0 => Ok(()),
            _ => Err(error::ErrorInternalServerError(
                "failed to remove session from cache",
            )),
        }
    }

    /// Invalidates session cookie.
    fn remove_cookie<B>(&self, res: &mut ServiceResponse<B>) -> Result<(), Error> {
        let mut cookie = Cookie::named(self.name.clone());
        cookie.set_value("");
        cookie.set_max_age(Duration::ZERO);
        cookie.set_expires(OffsetDateTime::now_utc() - Duration::days(365));

        let val =
            HeaderValue::from_str(&cookie.to_string()).map_err(error::ErrorInternalServerError)?;
        res.headers_mut().append(header::SET_COOKIE, val);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_session::Session;
    use actix_web::{
        middleware, web,
        web::{get, post, resource},
        App, HttpResponse, Result,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct IndexResponse {
        user_id: Option<String>,
        counter: i32,
    }

    async fn index(session: Session) -> Result<HttpResponse> {
        let user_id: Option<String> = session.get::<String>("user_id").unwrap();
        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
    }

    async fn do_something(session: Session) -> Result<HttpResponse> {
        let user_id: Option<String> = session.get::<String>("user_id").unwrap();
        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .map_or(1, |inner| inner + 1);
        session.insert("counter", &counter)?;

        Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
    }

    #[derive(Deserialize)]
    struct Identity {
        user_id: String,
    }

    async fn login(user_id: web::Json<Identity>, session: Session) -> Result<HttpResponse> {
        let id = user_id.into_inner().user_id;
        session.insert("user_id", &id)?;
        session.renew();

        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(HttpResponse::Ok().json(&IndexResponse {
            user_id: Some(id),
            counter,
        }))
    }

    async fn logout(session: Session) -> Result<HttpResponse> {
        let id: Option<String> = session.get("user_id")?;

        let body = if let Some(x) = id {
            session.purge();
            format!("Logged out: {}", x)
        } else {
            "Could not log out anonymous user".to_owned()
        };

        Ok(HttpResponse::Ok().body(body))
    }

    #[actix_rt::test]
    async fn test_session_workflow() {
        // Step 1:  GET index
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 2: POST to do_something
        //   - adds new session state in redis:  {"counter": 1}
        //   - set-cookie actix-session should be in response (session cookie #1)
        //   - response should be: {"counter": 1, "user_id": None}
        // Step 3:  GET index, including session cookie #1 in request
        //   - set-cookie will *not* be in response
        //   - response should be: {"counter": 1, "user_id": None}
        // Step 4: POST again to do_something, including session cookie #1 in request
        //   - updates session state in redis:  {"counter": 2}
        //   - response should be: {"counter": 2, "user_id": None}
        // Step 5: POST to login, including session cookie #1 in request
        //   - set-cookie actix-session will be in response  (session cookie #2)
        //   - updates session state in redis: {"counter": 2, "user_id": "ferris"}
        // Step 6: GET index, including session cookie #2 in request
        //   - response should be: {"counter": 2, "user_id": "ferris"}
        // Step 7: POST again to do_something, including session cookie #2 in request
        //   - updates session state in redis: {"counter": 3, "user_id": "ferris"}
        //   - response should be: {"counter": 3, "user_id": "ferris"}
        // Step 8: GET index, including session cookie #1 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 9: POST to logout, including session cookie #2
        //   - set-cookie actix-session will be in response with session cookie #2
        //     invalidation logic
        // Step 10: GET index, including session cookie #2 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}

        let srv = actix_test::start(|| {
            App::new()
                .wrap(RedisSession::new("127.0.0.1:6379", &[0; 32]).cookie_name("test-session"))
                .wrap(middleware::Logger::default())
                .service(resource("/").route(get().to(index)))
                .service(resource("/do_something").route(post().to(do_something)))
                .service(resource("/login").route(post().to(login)))
                .service(resource("/logout").route(post().to(logout)))
        });

        // Step 1:  GET index
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_1a = srv.get("/").send();
        let mut resp_1 = req_1a.await.unwrap();
        assert!(resp_1.cookies().unwrap().is_empty());
        let result_1 = resp_1.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_1,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );

        // Step 2: POST to do_something
        //   - adds new session state in redis:  {"counter": 1}
        //   - set-cookie actix-session should be in response (session cookie #1)
        //   - response should be: {"counter": 1, "user_id": None}
        let req_2 = srv.post("/do_something").send();
        let resp_2 = req_2.await.unwrap();
        let cookie_1 = resp_2
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_eq!(cookie_1.max_age(), Some(Duration::days(7)));

        // Step 3:  GET index, including session cookie #1 in request
        //   - set-cookie will *not* be in response
        //   - response should be: {"counter": 1, "user_id": None}
        let req_3 = srv.get("/").cookie(cookie_1.clone()).send();
        let mut resp_3 = req_3.await.unwrap();
        assert!(resp_3.cookies().unwrap().is_empty());
        let result_3 = resp_3.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_3,
            IndexResponse {
                user_id: None,
                counter: 1
            }
        );

        // Step 4: POST again to do_something, including session cookie #1 in request
        //   - updates session state in redis:  {"counter": 2}
        //   - response should be: {"counter": 2, "user_id": None}
        let req_4 = srv.post("/do_something").cookie(cookie_1.clone()).send();
        let mut resp_4 = req_4.await.unwrap();
        let result_4 = resp_4.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_4,
            IndexResponse {
                user_id: None,
                counter: 2
            }
        );

        // Step 5: POST to login, including session cookie #1 in request
        //   - set-cookie actix-session will be in response  (session cookie #2)
        //   - updates session state in redis: {"counter": 2, "user_id": "ferris"}
        let req_5 = srv
            .post("/login")
            .cookie(cookie_1.clone())
            .send_json(&json!({"user_id": "ferris"}));
        let mut resp_5 = req_5.await.unwrap();
        let cookie_2 = resp_5
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_ne!(cookie_1.value(), cookie_2.value());

        let result_5 = resp_5.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_5,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 2
            }
        );

        // Step 6: GET index, including session cookie #2 in request
        //   - response should be: {"counter": 2, "user_id": "ferris"}
        let req_6 = srv.get("/").cookie(cookie_2.clone()).send();
        let mut resp_6 = req_6.await.unwrap();
        let result_6 = resp_6.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_6,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 2
            }
        );

        // Step 7: POST again to do_something, including session cookie #2 in request
        //   - updates session state in redis: {"counter": 3, "user_id": "ferris"}
        //   - response should be: {"counter": 3, "user_id": "ferris"}
        let req_7 = srv.post("/do_something").cookie(cookie_2.clone()).send();
        let mut resp_7 = req_7.await.unwrap();
        let result_7 = resp_7.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_7,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 3
            }
        );

        // Step 8: GET index, including session cookie #1 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_8 = srv.get("/").cookie(cookie_1.clone()).send();
        let mut resp_8 = req_8.await.unwrap();
        assert!(resp_8.cookies().unwrap().is_empty());
        let result_8 = resp_8.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_8,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );

        // Step 9: POST to logout, including session cookie #2
        //   - set-cookie actix-session will be in response with session cookie #2
        //     invalidation logic
        let req_9 = srv.post("/logout").cookie(cookie_2.clone()).send();
        let resp_9 = req_9.await.unwrap();
        let cookie_3 = resp_9
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_ne!(
            OffsetDateTime::now_utc().year(),
            cookie_3
                .expires()
                .map(|t| t.datetime().expect("Expiration is a datetime").year())
                .unwrap()
        );

        // Step 10: GET index, including session cookie #2 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_10 = srv.get("/").cookie(cookie_2.clone()).send();
        let mut resp_10 = req_10.await.unwrap();
        assert!(resp_10.cookies().unwrap().is_empty());
        let result_10 = resp_10.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_10,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );
    }

    #[actix_rt::test]
    async fn test_max_age_session_only() {
        //
        // Test that removing max_age results in a session-only cookie
        //
        let srv = actix_test::start(|| {
            App::new()
                .wrap(
                    RedisSession::new("127.0.0.1:6379", &[0; 32])
                        .cookie_name("test-session")
                        .cookie_max_age(None),
                )
                .wrap(middleware::Logger::default())
                .service(resource("/do_something").route(post().to(do_something)))
        });

        let req = srv.post("/do_something").send();
        let resp = req.await.unwrap();
        let cookie = resp
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();

        assert_eq!(cookie.max_age(), None);
    }
}
