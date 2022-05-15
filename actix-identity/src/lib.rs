//! Identity management for Actix Web.
//!
//! `actix-identity` can be used to track identity of a user across multiple requests.
//! It is built on top of HTTP sessions, via [`actix-session`](https:://docs.rs/actix-session).
//!
//! # Getting started
//!
//! To start using identity management in your Actix Web application you must register [`IdentityMiddleware`]
//! and `SessionMiddleware` as middlewares on your `App`:
//!
//! ```no_run
//! use actix_web::{web, App, HttpServer};
//! # use actix_web::HttpResponse;
//! use actix_web::cookie::Key;
//! use actix_identity::IdentityMiddleware;
//! use actix_session::SessionMiddleware;
//! use actix_session::storage::RedisSessionStore;
//!
//! #[actix_web::main]
//! async fn main() {
//!     let secret_key = Key::generate();
//!     let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379")
//!         .await
//!         .unwrap();
//!     HttpServer::new(move || {
//!        App::new()
//!            // Install the identity framework.
//!            .wrap(IdentityMiddleware::default())
//!            // The identity system is built on top of sessions.
//!            // You must install the session middleware to leverage
//!            // `actix-identity`.
//!            // The session middleware must be mounted AFTER the
//!            // identity middleware: `actix-web` invokes middlewares
//!            // in the OPPOSITE order of registration when it receives
//!            // an incoming request.
//!            .wrap(SessionMiddleware::new(
//!                 redis_store.clone(),
//!                 secret_key.clone()
//!            ))
//!            // Your request handlers [...]
//!            # .default_service(web::to(|| HttpResponse::Ok()))
//!     })
//! # ;
//! }
//! ```
//!
//! User identities can be created, accessed and destroyed using the [`Identity`] extractor in your
//! request handlers:
//!
//! ```no_run
//! use actix_web::{
//!     HttpResponse, Responder, HttpRequest, get, post, HttpMessage
//! };
//! use actix_identity::Identity;
//! use actix_session::storage::RedisSessionStore;
//!
//! #[get("/")]
//! async fn index(user: Option<Identity>) -> impl Responder {
//!     if let Some(user) = user {
//!         format!("Welcome! {}", user.id().unwrap())
//!     } else {
//!         "Welcome Anonymous!".to_owned()
//!     }
//! }
//!
//! #[post("/login")]
//! async fn login(request: HttpRequest) -> impl Responder {
//!     // Some kind of authentication should happen here -
//!     // e.g. password-based, biometric, etc.
//!     // [...]
//!
//!     // Attached a verified user identity to the active
//!     // session.
//!     Identity::login(&request.extensions(), "User1".into());
//!     HttpResponse::Ok()
//! }
//!
//! #[post("/logout")]
//! async fn logout(user: Identity) -> impl Responder {
//!     user.logout();
//!     HttpResponse::Ok()
//! }
//! ```
//!
//! # Advanced configuration
//!
//! By default, `actix-identity` does not automatically log out users.
//! You can change this behaviour by customising the configuration for [`IdentityMiddleware`] via
//! [`IdentityMiddleware::builder`].
//!
//! In particular, you can automatically log out users:
//!
//! - who have been inactive for a while (see [`IdentityMiddlewareBuilder::visit_deadline`](configuration::IdentityMiddlewareBuilder::visit_deadline);
//! - who logged in too long ago (see [`IdentityMiddlewareBuilder::login_deadline`](configuration::IdentityMiddlewareBuilder::login_deadline)).
#![deny(rust_2018_idioms, nonstandard_style, missing_docs)]
#![warn(future_incompatible)]
pub mod configuration;
mod identity;
mod identity_ext;
mod middleware;

pub use self::identity::Identity;
pub use self::identity_ext::IdentityExt;
pub use self::middleware::IdentityMiddleware;
