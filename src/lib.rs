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
