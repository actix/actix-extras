//! HTTP authorization routines for [actix-web](https://github.com/actix/actix-web) framework.
//!
//! Currently supported schemas:
//!  * Basic ([RFC-7617](https://tools.ietf.org/html/rfc7617))

extern crate bytes;
extern crate percent_encoding;
extern crate actix_web;
extern crate base64;

mod errors;
pub mod basic;
