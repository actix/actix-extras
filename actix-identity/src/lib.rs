//! Identity management for Actix Web.
//!
//! // TODO: blurb on identity management + link to middleware
//!
//! Use the [`Identity`] extractor to access the user identity attached to the current session, if any.
//!
//! ```no_run
//! use actix_web::*;
//! use actix_web::cookie::Key;
//! use actix_identity::{Identity, IdentityMiddleware};
//! use actix_session::{Session, SessionMiddleware};
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
//!     Identity::login(&request.extensions(), "User1".into());
//!     HttpResponse::Ok()
//! }
//!
//! #[post("/logout")]
//! async fn logout(user: Identity) -> impl Responder {
//!     user.logout();
//!     HttpResponse::Ok()
//! }
//!
//! #[actix_web::main]
//! async fn main() {
//!     let secret_key = Key::generate();
//!     let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379").await.unwrap();
//!     HttpServer::new(move || {
//!        App::new()
//!            // Install the identity framework.
//!            .wrap(IdentityMiddleware::default())
//!            // The identity system is built on top of sessions.
//!            // You must install the session middleware to leverage `actix-identity`.
//!            // The session middleware must be mounted AFTER the identity middleware:
//!            // `actix-web` invokes middlewares in the OPPOSITE order of registration when
//!            // it receives an incoming request.
//!            .wrap(SessionMiddleware::new(redis_store.clone(), secret_key.clone()))
//!            .service(services![index, login, logout])
//!     })
//! # ;
//! }
//! ```
#![deny(rust_2018_idioms, nonstandard_style, missing_docs)]
#![warn(future_incompatible)]
pub mod configuration;
mod identity;
mod identity_ext;
mod middleware;

pub use self::identity::Identity;
pub use self::identity_ext::IdentityExt;
pub use self::middleware::IdentityMiddleware;
