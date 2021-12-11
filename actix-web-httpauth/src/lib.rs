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

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible, missing_docs)]

pub mod extractors;
pub mod headers;
pub mod middleware;
mod utils;
