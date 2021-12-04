//! Sessions for Actix Web.
//!
//! Provides a general solution for session management. Session middleware could provide different
//! implementations which could be accessed via general session API.
//!
//! This crate provides a general solution for session management and includes a cookie backend.
//! Other backend implementations can be built to use persistent or key-value stores, for example.
//!
//! In general, some session middleware, such as a [`CookieSession`] is initialized and applied.
//! To access session data, the [`Session`] extractor must be used. This extractor allows reading
//! modifying session data.
//!
//! ```no_run
//! use actix_web::{web, App, HttpServer, HttpResponse, Error};
//! use actix_session::{Session, CookieSession};
//!
//! fn index(session: Session) -> Result<&'static str, Error> {
//!     // access session data
//!     if let Some(count) = session.get::<i32>("counter")? {
//!         println!("SESSION value: {}", count);
//!         session.insert("counter", count + 1)?;
//!     } else {
//!         session.insert("counter", 1)?;
//!     }
//!
//!     Ok("Welcome!")
//! }
//!
//! #[actix_rt::main]
//! async fn main() -> std::io::Result<()> {
//!     HttpServer::new(
//!         || App::new()
//!             // create cookie based session middleware
//!             .wrap(CookieSession::signed(&[0; 32]).secure(false))
//!             .default_service(web::to(|| HttpResponse::Ok())))
//!         .bind(("127.0.0.1", 8080))?
//!         .run()
//!         .await
//! }
//! ```

#![deny(rust_2018_idioms, nonstandard_style)]
// #![warn(missing_docs)]

#[cfg(feature = "cookie-session")]
pub use storage::CookieSessionStore;
#[cfg(feature = "redis-actor-session")]
pub use storage::RedisActorSession;

pub use extractors::UserSession;
pub use middleware::{CookieContentSecurity, SessionMiddleware, SessionMiddlewareBuilder};
pub use session::{Session, SessionStatus};

mod extractors;
mod middleware;
mod session;
mod storage;
