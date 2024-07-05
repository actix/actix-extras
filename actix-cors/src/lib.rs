//! Cross-Origin Resource Sharing (CORS) controls for Actix Web.
//!
//! This middleware can be applied to both applications and resources. Once built, a [`Cors`]
//! builder can be used as an argument for Actix Web's `App::wrap()`, `Scope::wrap()`, or
//! `Resource::wrap()` methods.
//!
//! This CORS middleware automatically handles `OPTIONS` preflight requests.
//!
//! # Crate Features
//! - `draft-private-network-access`: ⚠️ Unstable. Adds opt-in support for the [Private Network
//!   Access] spec extensions. This feature is unstable since it will follow breaking changes in the
//!   draft spec until it is finalized.
//!
//! # Example
//! ```no_run
//! use actix_cors::Cors;
//! use actix_web::{get, http, web, App, HttpRequest, HttpResponse, HttpServer};
//!
//! #[get("/index.html")]
//! async fn index(req: HttpRequest) -> &'static str {
//!     "<p>Hello World!</p>"
//! }
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     HttpServer::new(|| {
//!         let cors = Cors::default()
//!             .allowed_origin("https://www.rust-lang.org")
//!             .allowed_origin_fn(|origin, _req_head| {
//!                 origin.as_bytes().ends_with(b".rust-lang.org")
//!             })
//!             .allowed_methods(vec!["GET", "POST"])
//!             .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
//!             .allowed_header(http::header::CONTENT_TYPE)
//!             .max_age(3600);
//!
//!         App::new()
//!             .wrap(cors)
//!             .service(index)
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await;
//!
//!     Ok(())
//! }
//! ```
//!
//! [Private Network Access]: https://wicg.github.io/private-network-access

#![forbid(unsafe_code)]
#![warn(future_incompatible, missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod all_or_some;
mod builder;
mod error;
mod inner;
mod middleware;

use crate::{
    all_or_some::AllOrSome,
    inner::{Inner, OriginFn},
};
pub use crate::{builder::Cors, error::CorsError, middleware::CorsMiddleware};
