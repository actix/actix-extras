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
//!            .wrap(SessionMiddleware::new(redis_store.clone(), secret_key.clone()))
//!            .service(services![index, login, logout])
//!     })
//! # ;
//! }
//! ```

#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible)]

pub mod configuration;
mod identity;
mod identity_ext;
mod middleware;

pub use self::identity::Identity;
pub use self::identity_ext::IdentityExt;
pub use self::middleware::IdentityMiddleware;

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use actix_web::{
        body::{BoxBody, EitherBody},
        dev::ServiceResponse,
        test, web, App, Error,
    };

    use super::*;

    pub(crate) const COOKIE_KEY_MASTER: [u8; 32] = [0; 32];
    pub(crate) const COOKIE_NAME: &str = "actix_auth";
    pub(crate) const COOKIE_LOGIN: &str = "test";

    #[allow(clippy::enum_variant_names)]
    pub(crate) enum LoginTimestampCheck {
        NoTimestamp,
        NewTimestamp,
        OldTimestamp(SystemTime),
    }

    #[allow(clippy::enum_variant_names)]
    pub(crate) enum VisitTimeStampCheck {
        NoTimestamp,
        NewTimestamp,
    }

    pub(crate) async fn create_identity_server<
        F: Fn(CookieIdentityPolicy) -> CookieIdentityPolicy + Sync + Send + Clone + 'static,
    >(
        f: F,
    ) -> impl actix_service::Service<
        actix_http::Request,
        Response = ServiceResponse<EitherBody<BoxBody>>,
        Error = Error,
    > {
        test::init_service(
            App::new()
                .wrap(IdentityMiddleware::new(f(CookieIdentityPolicy::new(
                    &COOKIE_KEY_MASTER,
                )
                .secure(false)
                .name(COOKIE_NAME))))
                .service(web::resource("/").to(|id: Identity| async move {
                    let identity = id.id();
                    if identity.is_none() {
                        id.remember(COOKIE_LOGIN.to_string())
                    }
                    web::Json(identity)
                })),
        )
        .await
    }
}
