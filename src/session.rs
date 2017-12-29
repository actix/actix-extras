use std::{io, net};
use std::rc::Rc;
use std::time::Duration;
use std::collections::HashMap;

use serde_json;
use futures::Future;
use futures::future::{Either, ok as FutOk, err as FutErr};
use tokio_core::net::TcpStream;
use actix::prelude::*;
use actix_web::{error, Error, HttpRequest, HttpResponse};
use actix_web::middleware::{SessionImpl, SessionBackend, Response as MiddlewareResponse};

use redis::{Command, RedisActor};


/// Session that stores data in redis
pub struct RedisSession {
    changed: bool,
    inner: Rc<Inner>,
    state: HashMap<String, String>,
}

impl SessionImpl for RedisSession {

    fn get(&self, key: &str) -> Option<&str> {
        if let Some(s) = self.state.get(key) {
            Some(s)
        } else {
            None
        }
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

    fn write(&self, mut resp: HttpResponse) -> MiddlewareResponse {
        if self.changed {
            let _ = self.inner.update(&self.state);
        }
        MiddlewareResponse::Done(resp)
    }
}

pub struct RedisSessionBackend(Rc<Inner>);

impl RedisSessionBackend {
    /// Create new redis session backend
    pub fn new<S: net::ToSocketAddrs>(addr: S, ttl: Duration) -> io::Result<RedisSessionBackend> {
        let h = Arbiter::handle();
        let mut err = None;
        for addr in addr.to_socket_addrs()? {
            match TcpStream::connect(&addr, &h).wait() {
                Err(e) => err = Some(e),
                Ok(conn) => {
                    let addr = RedisActor::start(conn);
                    return Ok(RedisSessionBackend(Rc::new(Inner{ttl: ttl, addr: addr})));
                },
            }
        }
        if let Some(e) = err.take() {
            Err(e)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Can not connect to redis server."))
        }
    }
}

impl<S> SessionBackend<S> for RedisSessionBackend {

    type Session = RedisSession;
    type ReadFuture = Box<Future<Item=RedisSession, Error=Error>>;

    fn from_request(&self, req: &mut HttpRequest<S>) -> Self::ReadFuture {
        let inner = Rc::clone(&self.0);

        Box::new(self.0.load(req).map(move |state| {
            if let Some(state) = state {
                RedisSession {
                    changed: false,
                    inner: inner,
                    state: state,
                }
            } else {
                RedisSession {
                    changed: false,
                    inner: inner,
                    state: HashMap::new(),
                }
            }
        }))
    }
}

struct Inner {
    ttl: Duration,
    addr: Address<RedisActor>,
}

impl Inner {
    fn load<S>(&self, req: &mut HttpRequest<S>)
               -> Box<Future<Item=Option<HashMap<String, String>>, Error=Error>>
    {
        if let Ok(cookies) = req.cookies() {
            for cookie in cookies {
                if cookie.name() == "actix-session" {
                    return Box::new(
                        self.addr.call_fut(Command(resp_array!["GET", cookie.value()]))
                            .map_err(Error::from)
                            .and_then(|res| {
                                match res {
                                    Ok(val) => {
                                        println!("VAL {:?}", val);
                                        Ok(Some(HashMap::new()))
                                    },
                                    Err(err) => Err(
                                        io::Error::new(io::ErrorKind::Other, "Error").into())
                                }
                            }))
                }
            }
        }
        Box::new(FutOk(None))
    }

    fn update(&self, state: &HashMap<String, String>) -> Box<Future<Item=(), Error=Error>> {
        Box::new(
            match serde_json::to_string(state) {
                Err(e) => Either::A(FutErr(e.into())),
                Ok(body) => {
                    Either::B(
                        self.addr.call_fut(Command(resp_array!["GET", "test"]))
                            .map_err(Error::from)
                            .and_then(|res| {
                                match res {
                                    Ok(val) => Ok(()),
                                    Err(err) => Err(
                                        error::ErrorInternalServerError(err).into())
                                }
                            }))
                }
            })
    }
}
