//! Identity management for Actix Web.
//!
//! `actix-identity` can be used to track identity of a user across multiple requests. It is built
//! on top of HTTP sessions, via [`actix-session`](https://docs.rs/actix-session).
//!
//! # Getting started
//! To start using identity management in your Actix Web application you must register
//! [`IdentityMiddleware`] and `SessionMiddleware` as middleware on your `App`:
//!
//! ```no_run
//! # use actix_web::web;
//! use actix_web::{cookie::Key, App, HttpServer, HttpResponse};
//! use actix_identity::IdentityMiddleware;
//! use actix_session::{storage::RedisSessionStore, SessionMiddleware};
//!
//! #[actix_web::main]
//! async fn main() {
//!     let secret_key = Key::generate();
//!     let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379")
//!         .await
//!         .unwrap();
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             // Install the identity framework first.
//!             .wrap(IdentityMiddleware::default())
//!             // The identity system is built on top of sessions. You must install the session
//!             // middleware to leverage `actix-identity`. The session middleware must be mounted
//!             // AFTER the identity middleware: `actix-web` invokes middleware in the OPPOSITE
//!             // order of registration when it receives an incoming request.
//!             .wrap(SessionMiddleware::new(
//!                  redis_store.clone(),
//!                  secret_key.clone()
//!             ))
//!             // Your request handlers [...]
//!             # .default_service(web::to(|| HttpResponse::Ok()))
//!     })
//! # ;
//! }
//! ```
//!
//! User identities can be created, accessed and destroyed using the [`Identity`] extractor in your
//! request handlers:
//!
//! ```no_run
//! use actix_web::{get, post, HttpResponse, Responder, HttpRequest, HttpMessage};
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
//!     // Some kind of authentication should happen here
//!     // e.g. password-based, biometric, etc.
//!     // [...]
//!
//!     // attach a verified user identity to the active session
//!     Identity::login(&request.extensions(), "User1".into()).unwrap();
//!
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
//! By default, `actix-identity` does not automatically log out users. You can change this behaviour
//! by customising the configuration for [`IdentityMiddleware`] via [`IdentityMiddleware::builder`].
//!
//! In particular, you can automatically log out users who:
//! - have been inactive for a while (see [`IdentityMiddlewareBuilder::visit_deadline`];
//! - logged in too long ago (see [`IdentityMiddlewareBuilder::login_deadline`]).
//!
//! [`IdentityMiddlewareBuilder::visit_deadline`]: config::IdentityMiddlewareBuilder::visit_deadline
//! [`IdentityMiddlewareBuilder::login_deadline`]: config::IdentityMiddlewareBuilder::login_deadline

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style, missing_docs)]
#![warn(future_incompatible)]

pub mod config;
mod identity;
mod identity_ext;
mod middleware;

pub use self::identity::Identity;
pub use self::identity_ext::IdentityExt;
pub use self::middleware::IdentityMiddleware;
