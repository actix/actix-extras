extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate futures;
#[macro_use]
extern crate failure;
extern crate env_logger;

#[cfg(test)]
extern crate http;

extern crate prost;
#[cfg(test)]
#[macro_use] extern crate prost_derive;

mod use_prost;
pub use use_prost::{ ProtoBuf, ProtoBufResponseBuilder, ProtoBufHttpMessage };