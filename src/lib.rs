//! HTTP Authorization support for [actix-web](https://actix.rs) framework.
//!
//! Provides:
//!  * typed [Authorization] and  [WWW-Authenticate] headers
//!  * [extractors] for an [Authorization] header
//!  * [middleware] for easier authorization checking
//!
//! ## Supported schemes
//!
//!  * `Basic`, as defined in [RFC7617](https://tools.ietf.org/html/rfc7617)
//!  * `Bearer`, as defined in [RFC6750](https://tools.ietf.org/html/rfc6750)
//!
//! [Authorization]: `crate::headers::authorization::Authorization`
//! [WWW-Authenticate]: `crate::headers::www_authenticate::WwwAuthenticate`
//! [extractors]: https://actix.rs/docs/extractors/
//! [middleware]: ./middleware/

#![deny(bare_trait_objects)]
#![deny(missing_docs)]
#![deny(unused)]
#![cfg_attr(feature = "nightly", feature(test))]

pub mod extractors;
pub mod headers;
pub mod middleware;
mod utils;
