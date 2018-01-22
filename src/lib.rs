extern crate actix;
extern crate backoff;
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
#[macro_use]
extern crate log;
#[macro_use]
extern crate redis_async;
#[macro_use]
extern crate failure;
extern crate trust_dns_resolver;

mod redis;
mod connect;

pub use redis::RedisActor;
pub use connect::TcpConnector;

#[cfg(feature="web")]
extern crate actix_web;
#[cfg(feature="web")]
extern crate cookie;
#[cfg(feature="web")]
extern crate rand;
#[cfg(feature="web")]
extern crate http;
#[cfg(feature="web")]
extern crate serde;
#[cfg(feature="web")]
extern crate serde_json;

#[cfg(feature="web")]
mod session;
#[cfg(feature="web")]
pub use session::RedisSessionBackend;


#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display="Redis error {}", _0)]
    Redis(redis_async::error::Error),
    /// Receiving message during reconnecting
    #[fail(display="Redis: Not connected")]
    NotConnected,
    /// Cancel all waters when connection get dropped
    #[fail(display="Redis: Disconnected")]
    Disconnected,
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl From<redis_async::error::Error> for Error {
    fn from(err: redis_async::error::Error) -> Error {
        Error::Redis(err)
    }
}
