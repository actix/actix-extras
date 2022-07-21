//! HTTP authentication schemes for [Actix Web](https://actix.rs).
//!
//! Provides:
//! - Typed [Authorization] and [WWW-Authenticate] headers
//! - [Extractors] for an [Authorization] header
//! - [Middleware] for easier authorization checking
//!
//! ## Supported schemes
//! - `Bearer` as defined in [RFC 6750](https://tools.ietf.org/html/rfc6750).
//! - `Basic` as defined in [RFC 7617](https://tools.ietf.org/html/rfc7617).
//!
//! [Authorization]: `self::headers::authorization::Authorization`
//! [WWW-Authenticate]: `self::headers::www_authenticate::WwwAuthenticate`
//! [Extractors]: https://actix.rs/docs/extractors
//! [Middleware]: self::middleware

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible, missing_docs)]

pub mod extractors;
pub mod headers;
pub mod middleware;
mod utils;
