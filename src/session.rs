use std::collections::HashMap;
use std::iter::FromIterator;
use std::rc::Rc;

use actix::prelude::*;
use actix_web::middleware::session::{SessionBackend, SessionImpl};
use actix_web::middleware::Response as MiddlewareResponse;
use actix_web::{error, Error, HttpRequest, HttpResponse, Result};
use cookie::{Cookie, CookieJar, Key, SameSite};
use futures::future::{err as FutErr, ok as FutOk, Either};
use futures::Future;
use http::header::{self, HeaderValue};
use rand::{self, Rng};
use redis_async::resp::RespValue;
use serde_json;
use time::Duration;

use redis::{Command, RedisActor};

/// Session that stores data in redis
pub struct RedisSession {
    changed: bool,
    inner: Rc<Inner>,
    state: HashMap<String, String>,
    value: Option<String>,
}

impl SessionImpl for RedisSession {
    fn get(&self, key: &str) -> Option<&str> {
        self.state.get(key).map(|s| s.as_str())
    }

    fn set(&mut self, key: &str, value: String) {
        self.changed = true;
        self.state.insert(key.to_owned(), value);
    }

    fn remove(&mut self, key: &str) {
        self.changed = true;
        self.state.remove(key);
    }

    fn clear(&mut self) {
        self.changed = true;
        self.state.clear()
    }

    fn write(&self, resp: HttpResponse) -> Result<MiddlewareResponse> {
        if self.changed {
            Ok(MiddlewareResponse::Future(self.inner.update(
                &self.state,
                resp,
                self.value.as_ref(),
            )))
        } else {
            Ok(MiddlewareResponse::Done(resp))
        }
    }
}

/// Use redis as session storage.
///
/// You need to pass an address of the redis server and random value to the
/// constructor of `RedisSessionBackend`. This is private key for cookie
/// session, When this value is changed, all session data is lost.
///
/// Note that whatever you write into your session is visible by the user (but
/// not modifiable).
///
/// Constructor panics if key length is less than 32 bytes.
pub struct RedisSessionBackend(Rc<Inner>);

impl RedisSessionBackend {
    /// Create new redis session backend
    ///
    /// * `addr` - address of the redis server
    pub fn new<S: Into<String>>(addr: S, key: &[u8]) -> RedisSessionBackend {
        RedisSessionBackend(Rc::new(Inner {
            key: Key::from_master(key),
            ttl: "7200".to_owned(),
            addr: RedisActor::start(addr),
            name: "actix-session".to_owned(),
            path: "/".to_owned(),
            domain: None,
            secure: false,
            max_age: None,
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
}

impl<S> SessionBackend<S> for RedisSessionBackend {
    type Session = RedisSession;
    type ReadFuture = Box<Future<Item = RedisSession, Error = Error>>;

    fn from_request(&self, req: &mut HttpRequest<S>) -> Self::ReadFuture {
        let inner = Rc::clone(&self.0);

        Box::new(self.0.load(req).map(move |state| {
            if let Some((state, value)) = state {
                RedisSession {
                    changed: false,
                    inner: inner,
                    state: state,
                    value: Some(value),
                }
            } else {
                RedisSession {
                    changed: false,
                    inner: inner,
                    state: HashMap::new(),
                    value: None,
                }
            }
        }))
    }
}

struct Inner {
    key: Key,
    ttl: String,
    addr: Addr<Unsync, RedisActor>,
    name: String,
    path: String,
    domain: Option<String>,
    secure: bool,
    max_age: Option<Duration>,
    same_site: Option<SameSite>,
}

impl Inner {
    #[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
    fn load<S>(
        &self, req: &mut HttpRequest<S>,
    ) -> Box<Future<Item = Option<(HashMap<String, String>, String)>, Error = Error>>
    {
        if let Ok(cookies) = req.cookies() {
            for cookie in cookies {
                if cookie.name() == self.name {
                    let mut jar = CookieJar::new();
                    jar.add_original(cookie.clone());
                    if let Some(cookie) = jar.signed(&self.key).get(&self.name) {
                        let value = cookie.value().to_owned();
                        return Box::new(
                            self.addr
                                .send(Command(resp_array!["GET", cookie.value()]))
                                .map_err(Error::from)
                                .and_then(move |res| match res {
                                    Ok(val) => {
                                        match val {
                                            RespValue::Error(err) => {
                                                return Err(
                                                    error::ErrorInternalServerError(err)
                                                        .into(),
                                                )
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
                                        Err(error::ErrorInternalServerError(err).into())
                                    }
                                }),
                        );
                    } else {
                        return Box::new(FutOk(None));
                    }
                }
            }
        }
        Box::new(FutOk(None))
    }

    fn update(
        &self, state: &HashMap<String, String>, mut resp: HttpResponse,
        value: Option<&String>,
    ) -> Box<Future<Item = HttpResponse, Error = Error>> {
        let (value, jar) = if let Some(value) = value {
            (value.clone(), None)
        } else {
            let mut rng = rand::OsRng::new().unwrap();
            let value = String::from_iter(rng.gen_ascii_chars().take(32));

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

        Box::new(match serde_json::to_string(state) {
            Err(e) => Either::A(FutErr(e.into())),
            Ok(body) => Either::B(
                self.addr
                    .send(Command(resp_array![
                        "SET", value, body, "EX", &self.ttl
                    ]))
                    .map_err(Error::from)
                    .and_then(move |res| match res {
                        Ok(_) => {
                            if let Some(jar) = jar {
                                for cookie in jar.delta() {
                                    let val =
                                        HeaderValue::from_str(&cookie.to_string())?;
                                    resp.headers_mut().append(header::SET_COOKIE, val);
                                }
                            }
                            Ok(resp)
                        }
                        Err(err) => Err(error::ErrorInternalServerError(err).into()),
                    }),
            ),
        })
    }
}
