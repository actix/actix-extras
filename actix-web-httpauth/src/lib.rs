//! HTTP authentication schemes for [actix-web](https://actix.rs).
//!
//! Provides:
//! - Typed [Authorization] and [WWW-Authenticate] headers
//! - [Extractors] for an [Authorization] header
//! - [Middleware] for easier authorization checking
//!
//! ## Supported schemes
//!
//! - `Basic`, as defined in [RFC7617](https://tools.ietf.org/html/rfc7617)
//! - `Bearer`, as defined in [RFC6750](https://tools.ietf.org/html/rfc6750)
//!
//! [Authorization]: `crate::headers::authorization::Authorization`
//! [WWW-Authenticate]: `crate::headers::www_authenticate::WwwAuthenticate`
//! [Extractors]: https://actix.rs/docs/extractors/
//! [Middleware]: ./middleware

#![deny(missing_docs, nonstandard_style, rust_2018_idioms)]
#![deny(clippy::all)]

pub mod extractors;
pub mod headers;
pub mod middleware;
mod utils;
