//! HTTP Authorization support for [actix-web](https://actix.rs) framework.
//!
//! Provides [`Authorization`](./headers/authorization/struct.Authorization.html)
//! and  [`WWW-Authenticate`](./headers/www_authenticate/struct.WWWAuthenticate.html) headers,
//! and `actix-web` extractors for an `Authorization` header.

#![cfg_attr(feature = "nightly", feature(test))]
#[cfg(feature = "nightly")] extern crate test;

extern crate actix_web;
extern crate bytes;
extern crate base64;

pub mod headers;
pub mod extractors;
