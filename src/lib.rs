extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate futures;
#[macro_use]
extern crate failure;
extern crate env_logger;

#[cfg(test)]
extern crate http;

#[cfg(feature="use_prost")]
extern crate prost;
#[cfg(test)]
#[cfg(feature="use_prost")]
#[macro_use] extern crate prost_derive;

#[cfg(feature="use_prost")]
mod use_prost;
#[cfg(feature="use_prost")]
pub use use_prost::{ ProtoBuf, ProtoBufResponseBuilder, ProtoBufHttpMessage };