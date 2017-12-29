extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate cookie;
extern crate futures;
extern crate serde;
extern crate serde_json;
extern crate rand;
extern crate http;
extern crate tokio_io;
extern crate tokio_core;
#[macro_use]
extern crate redis_async;
#[macro_use]
extern crate failure;

mod redis;
mod session;

pub use redis::RedisActor;
pub use session::RedisSessionBackend;
