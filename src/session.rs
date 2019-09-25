use std::cell::RefCell;
use std::{collections::HashMap, iter, rc::Rc};

use actix::prelude::*;
use actix_service::{Service, Transform};
use actix_session::{Session, SessionStatus};
use actix_web::cookie::{Cookie, CookieJar, Key, SameSite};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{self, HeaderValue};
use actix_web::{error, Error, HttpMessage};
use futures::future::{err, ok, Either, Future, FutureResult};
use futures::Poll;
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use redis_async::resp::RespValue;
use time::{self, Duration};

use crate::redis::{Command, RedisActor};

/// Use redis as session storage.
///
/// You need to pass an address of the redis server and random value to the
/// constructor of `RedisSessionBackend`. This is private key for cookie
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
            key: Key::from_master(key),
            cache_keygen: Box::new(|key: &str| format!("session:{}", &key)),
            ttl: "7200".to_owned(),
            addr: RedisActor::start(addr),
            name: "actix-session".to_owned(),
            path: "/".to_owned(),
            domain: None,
            secure: false,
            max_age: Some(Duration::days(7)),
            same_site: None,
        }))
    }

    /// Set time to live in seconds for session value
    pub fn ttl(mut self, ttl: u16) -> Self {
        Rc::get_mut(&mut self.0).unwrap().ttl = format!("{}", ttl);
        self
    }

    /// Set custom cookie name for session id
    pub fn cookie_name(mut self, name: &str) -> Self {
        Rc::get_mut(&mut self.0).unwrap().name = name.to_owned();
        self
    }

    /// Set custom cookie path
    pub fn cookie_path(mut self, path: &str) -> Self {
        Rc::get_mut(&mut self.0).unwrap().path = path.to_owned();
        self
    }

    /// Set custom cookie domain
    pub fn cookie_domain(mut self, domain: &str) -> Self {
        Rc::get_mut(&mut self.0).unwrap().domain = Some(domain.to_owned());
        self
    }

    /// Set custom cookie secure
    /// If the `secure` field is set, a cookie will only be transmitted when the
    /// connection is secure - i.e. `https`
    pub fn cookie_secure(mut self, secure: bool) -> Self {
        Rc::get_mut(&mut self.0).unwrap().secure = secure;
        self
    }

    /// Set custom cookie max-age
    pub fn cookie_max_age(mut self, max_age: Duration) -> Self {
        Rc::get_mut(&mut self.0).unwrap().max_age = Some(max_age);
        self
    }

    /// Set custom cookie SameSite
    pub fn cookie_same_site(mut self, same_site: SameSite) -> Self {
        Rc::get_mut(&mut self.0).unwrap().same_site = Some(same_site);
        self
    }

    /// Set a custom cache key generation strategy, expecting session key as input
    pub fn cache_keygen(mut self, keygen: Box<dyn Fn(&str) -> String>) -> Self {
        Rc::get_mut(&mut self.0).unwrap().cache_keygen = keygen;
        self
    }
}

impl<S, B> Transform<S> for RedisSession
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type InitError = ();
    type Transform = RedisSessionMiddleware<S>;
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RedisSessionMiddleware {
            service: Rc::new(RefCell::new(service)),
            inner: self.0.clone(),
        })
    }
}

/// Cookie session middleware
pub struct RedisSessionMiddleware<S: 'static> {
    service: Rc<RefCell<S>>,
    inner: Rc<Inner>,
}

impl<S, B> Service for RedisSessionMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.borrow_mut().poll_ready()
    }

    fn call(&mut self, mut req: ServiceRequest) -> Self::Future {
        let mut srv = self.service.clone();
        let inner = self.inner.clone();

        Box::new(self.inner.load(&req).and_then(move |state| {
            let value = if let Some((state, value)) = state {
                Session::set_session(state.into_iter(), &mut req);
                Some(value)
            } else {
                None
            };

            srv.call(req).and_then(move |mut res| {
                match Session::get_changes(&mut res) {
                    (SessionStatus::Unchanged, None) => {
                        Either::A(Either::A(Either::A(ok(res))))
                    }
                    (SessionStatus::Unchanged, Some(state)) => {
                        Either::A(Either::A(Either::B(if value.is_none() {
                            // implies the session is new
                            Either::A(inner.update(res, state, value))
                        } else {
                            Either::B(ok(res))
                        })))
                    }
                    (SessionStatus::Changed, Some(state)) => {
                        Either::A(Either::B(Either::A(inner.update(res, state, value))))
                    }
                    (SessionStatus::Purged, Some(_)) => {
                        if let Some(val) = value {
                            Either::A(Either::B(Either::B(Either::A(
                                inner.clear_cache(val).and_then(move |_| {
                                    match inner.remove_cookie(&mut res) {
                                        Ok(_) => Either::A(ok(res)),
                                        Err(_err) => Either::B(err(
                                            error::ErrorInternalServerError(_err),
                                        )),
                                    }
                                }),
                            ))))
                        } else {
                            Either::A(Either::B(Either::B(Either::B(err(
                                error::ErrorInternalServerError("unexpected"),
                            )))))
                        }
                    }
                    (SessionStatus::Renewed, Some(state)) => {
                        if let Some(val) = value {
                            Either::B(Either::A(
                                inner
                                    .clear_cache(val)
                                    .and_then(move |_| inner.update(res, state, None)),
                            ))
                        } else {
                            Either::B(Either::B(inner.update(res, state, None)))
                        }
                    }
                    (_, None) => unreachable!(),
                }
            })
        }))
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
}

impl Inner {
    fn load(
        &self,
        req: &ServiceRequest,
    ) -> impl Future<Item = Option<(HashMap<String, String>, String)>, Error = Error>
    {
        if let Ok(cookies) = req.cookies() {
            for cookie in cookies.iter() {
                if cookie.name() == self.name {
                    let mut jar = CookieJar::new();
                    jar.add_original(cookie.clone());
                    if let Some(cookie) = jar.signed(&self.key).get(&self.name) {
                        let value = cookie.value().to_owned();
                        let cachekey = (self.cache_keygen)(&cookie.value());
                        return Either::A(
                            self.addr
                                .send(Command(resp_array!["GET", cachekey]))
                                .map_err(Error::from)
                                .and_then(move |res| match res {
                                    Ok(val) => {
                                        match val {
                                            RespValue::Error(err) => {
                                                return Err(
                                                    error::ErrorInternalServerError(err),
                                                );
                                            }
                                            RespValue::SimpleString(s) => {
                                                if let Ok(val) = serde_json::from_str(&s)
                                                {
                                                    return Ok(Some((val, value)));
                                                }
                                            }
                                            RespValue::BulkString(s) => {
                                                if let Ok(val) =
                                                    serde_json::from_slice(&s)
                                                {
                                                    return Ok(Some((val, value)));
                                                }
                                            }
                                            _ => (),
                                        }
                                        Ok(None)
                                    }
                                    Err(err) => {
                                        Err(error::ErrorInternalServerError(err))
                                    }
                                }),
                        );
                    } else {
                        return Either::B(ok(None));
                    }
                }
            }
        }
        Either::B(ok(None))
    }

    fn update<B>(
        &self,
        mut res: ServiceResponse<B>,
        state: impl Iterator<Item = (String, String)>,
        value: Option<String>,
    ) -> impl Future<Item = ServiceResponse<B>, Error = Error> {
        let (value, jar) = if let Some(value) = value {
            (value.clone(), None)
        } else {
            let value: String = iter::repeat(())
                .map(|()| OsRng.sample(Alphanumeric))
                .take(32)
                .collect();

            // prepare session id cookie
            let mut cookie = Cookie::new(self.name.clone(), value.clone());
            cookie.set_path(self.path.clone());
            cookie.set_secure(self.secure);
            cookie.set_http_only(true);

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
            jar.signed(&self.key).add(cookie);

            (value, Some(jar))
        };

        let cachekey = (self.cache_keygen)(&value);

        let state: HashMap<_, _> = state.collect();
        match serde_json::to_string(&state) {
            Err(e) => Either::A(err(e.into())),
            Ok(body) => Either::B(
                self.addr
                    .send(Command(resp_array!["SET", cachekey, body, "EX", &self.ttl]))
                    .map_err(Error::from)
                    .and_then(move |redis_result| match redis_result {
                        Ok(_) => {
                            if let Some(jar) = jar {
                                for cookie in jar.delta() {
                                    let val =
                                        HeaderValue::from_str(&cookie.to_string())?;
                                    res.headers_mut().append(header::SET_COOKIE, val);
                                }
                            }
                            Ok(res)
                        }
                        Err(err) => Err(error::ErrorInternalServerError(err)),
                    }),
            ),
        }
    }

    /// removes cache entry
    fn clear_cache(&self, key: String) -> impl Future<Item = (), Error = Error> {
        let cachekey = (self.cache_keygen)(&key);

        self.addr
            .send(Command(resp_array!["DEL", cachekey]))
            .map_err(Error::from)
            .and_then(|res| {
                match res {
                    // redis responds with number of deleted records
                    Ok(RespValue::Integer(x)) if x > 0 => Ok(()),
                    _ => Err(error::ErrorInternalServerError(
                        "failed to remove session from cache",
                    )),
                }
            })
    }

    /// invalidates session cookie
    fn remove_cookie<B>(&self, res: &mut ServiceResponse<B>) -> Result<(), Error> {
        let mut cookie = Cookie::named(self.name.clone());
        cookie.set_value("");
        cookie.set_max_age(Duration::seconds(0));
        cookie.set_expires(time::now() - Duration::days(365));

        let val = HeaderValue::from_str(&cookie.to_string())
            .map_err(|err| error::ErrorInternalServerError(err))?;
        res.headers_mut().append(header::SET_COOKIE, val);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_http::{httpmessage::HttpMessage, HttpService};
    use actix_http_test::{block_on, TestServer};
    use actix_session::Session;
    use actix_web::{
        middleware, web,
        web::{get, post, resource},
        App, HttpResponse, HttpServer, Result,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use time;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct IndexResponse {
        user_id: Option<String>,
        counter: i32,
    }

    fn index(session: Session) -> Result<HttpResponse> {
        let user_id: Option<String> = session.get::<String>("user_id").unwrap();
        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(HttpResponse::Ok().json(IndexResponse { user_id, counter }))
    }

    fn do_something(session: Session) -> Result<HttpResponse> {
        let user_id: Option<String> = session.get::<String>("user_id").unwrap();
        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .map_or(1, |inner| inner + 1);
        session.set("counter", counter)?;

        Ok(HttpResponse::Ok().json(IndexResponse { user_id, counter }))
    }

    #[derive(Deserialize)]
    struct Identity {
        user_id: String,
    }
    fn login(user_id: web::Json<Identity>, session: Session) -> Result<HttpResponse> {
        let id = user_id.into_inner().user_id;
        session.set("user_id", &id)?;
        session.renew();

        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(HttpResponse::Ok().json(IndexResponse {
            user_id: Some(id),
            counter,
        }))
    }

    fn logout(session: Session) -> Result<HttpResponse> {
        let id: Option<String> = session.get("user_id")?;
        if let Some(x) = id {
            session.purge();
            Ok(format!("Logged out: {}", x).into())
        } else {
            Ok("Could not log out anonymous user".into())
        }
    }

    #[test]
    fn test_workflow() {
        // Step 1:  GET index
        //   - set-cookie actix-session will be in response (session cookie #1)
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 2:  GET index, including session cookie #1 in request
        //   - set-cookie will *not* be in response
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 3: POST to do_something, including session cookie #1 in request
        //   - adds new session state in redis:  {"counter": 1}
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
        //   - response should be: {"counter": 2, "user_id": None}
        // Step 8: GET index, including session cookie #1 in request
        //   - set-cookie actix-session will be in response (session cookie #3)
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 9: POST to logout, including session cookie #2
        //   - set-cookie actix-session will be in response with session cookie #2
        //     invalidation logic
        // Step 10: GET index, including session cookie #2 in request
        //   - set-cookie actix-session will be in response (session cookie #3)
        //   - response should be: {"counter": 0, "user_id": None}

        let mut srv = TestServer::new(|| {
            HttpService::new(
                App::new()
                    .wrap(
                        RedisSession::new("127.0.0.1:6379", &[0; 32])
                            .cookie_name("test-session"),
                    )
                    .wrap(middleware::Logger::default())
                    .service(resource("/").route(get().to(index)))
                    .service(resource("/do_something").route(post().to(do_something)))
                    .service(resource("/login").route(post().to(login)))
                    .service(resource("/logout").route(post().to(logout))),
            )
        });

        // Step 1:  GET index
        //   - set-cookie actix-session will be in response (session cookie #1)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_1a = srv.get("/").send();
        let mut resp_1 = srv.block_on(req_1a).unwrap();
        let cookie_1 = resp_1
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        let result_1 = block_on(resp_1.json::<IndexResponse>()).unwrap();
        assert_eq!(
            result_1,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );

        // Step 2:  GET index, including session cookie #1 in request
        //   - set-cookie will *not* be in response
        //   - response should be: {"counter": 0, "user_id": None}
        let req_2 = srv.get("/").cookie(cookie_1.clone()).send();
        let resp_2 = srv.block_on(req_2).unwrap();
        let cookie_2 = resp_2
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session");
        assert_eq!(cookie_2, None);

        // Step 3: POST to do_something, including session cookie #1 in request
        //   - adds new session state in redis:  {"counter": 1}
        //   - response should be: {"counter": 1, "user_id": None}
        let req_3 = srv.post("/do_something").cookie(cookie_1.clone()).send();
        let mut resp_3 = srv.block_on(req_3).unwrap();
        let result_3 = block_on(resp_3.json::<IndexResponse>()).unwrap();
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
        let mut resp_4 = srv.block_on(req_4).unwrap();
        let result_4 = block_on(resp_4.json::<IndexResponse>()).unwrap();
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
        let mut resp_5 = srv.block_on(req_5).unwrap();
        let cookie_2 = resp_5
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_eq!(
            true,
            cookie_1.value().to_string() != cookie_2.value().to_string()
        );

        let result_5 = block_on(resp_5.json::<IndexResponse>()).unwrap();
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
        let mut resp_6 = srv.block_on(req_6).unwrap();
        let result_6 = block_on(resp_6.json::<IndexResponse>()).unwrap();
        assert_eq!(
            result_6,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 2
            }
        );

        // Step 7: POST again to do_something, including session cookie #2 in request
        //   - updates session state in redis: {"counter": 3, "user_id": "ferris"}
        //   - response should be: {"counter": 2, "user_id": None}
        let req_7 = srv.post("/do_something").cookie(cookie_2.clone()).send();
        let mut resp_7 = srv.block_on(req_7).unwrap();
        let result_7 = block_on(resp_7.json::<IndexResponse>()).unwrap();
        assert_eq!(
            result_7,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 3
            }
        );

        // Step 8: GET index, including session cookie #1 in request
        //   - set-cookie actix-session will be in response (session cookie #3)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_8 = srv.get("/").cookie(cookie_1.clone()).send();
        let mut resp_8 = srv.block_on(req_8).unwrap();
        let cookie_3 = resp_8
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        let result_8 = block_on(resp_8.json::<IndexResponse>()).unwrap();
        assert_eq!(
            result_8,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );
        assert!(cookie_3.value().to_string() != cookie_2.value().to_string());

        // Step 9: POST to logout, including session cookie #2
        //   - set-cookie actix-session will be in response with session cookie #2
        //     invalidation logic
        let req_9 = srv.post("/logout").cookie(cookie_2.clone()).send();
        let resp_9 = srv.block_on(req_9).unwrap();
        let cookie_4 = resp_9
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert!(&time::now().tm_year != &cookie_4.expires().map(|t| t.tm_year).unwrap());

        // Step 10: GET index, including session cookie #2 in request
        //   - set-cookie actix-session will be in response (session cookie #3)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_10 = srv.get("/").cookie(cookie_2.clone()).send();
        let mut resp_10 = srv.block_on(req_10).unwrap();
        let result_10 = block_on(resp_10.json::<IndexResponse>()).unwrap();
        assert_eq!(
            result_10,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );

        let cookie_5 = resp_10
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert!(cookie_5.value().to_string() != cookie_2.value().to_string());
    }
}
