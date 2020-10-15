//! Cross-Origin Resource Sharing (CORS) controls for Actix Web.
//!
//! This middleware can be applied to both applications and resources. Once built,
//! [`CorsFactory`] can be used as a parameter for actix-web `App::wrap()`,
//! `Scope::wrap()`, or `Resource::wrap()` methods.
//!
//! This CORS middleware automatically handles `OPTIONS` preflight requests.
//!
//! # Example
//!
//! In this example a custom CORS middleware is registered for the "/index.html" endpoint.
//!
//! ```rust,no_run
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
//!         let cors = Cors::new()
//!               .allowed_origin("https://www.rust-lang.org/")
//!               .allowed_origin_fn(|req| {
//!                   req.headers
//!                       .get(http::header::ORIGIN)
//!                       .map(http::HeaderValue::as_bytes)
//!                       .filter(|b| b.ends_with(b".rust-lang.org"))
//!                       .is_some()
//!               })
//!               .allowed_methods(vec!["GET", "POST"])
//!               .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
//!               .allowed_header(http::header::CONTENT_TYPE)
//!               .max_age(3600)
//!               .finish();
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

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]

mod all_or_some;
mod builder;
mod error;
mod inner;
mod middleware;

pub use all_or_some::AllOrSome;
pub use builder::{Cors, CorsFactory};
pub use error::CorsError;
use inner::{Inner, OriginFn};
pub use middleware::CorsMiddleware;
