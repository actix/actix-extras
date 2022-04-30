//! Identity management for Actix Web.
//!
//! // TODO: blurb on identity management + link to middleware
//!
//! Use the [`Identity`] extractor to access the user identity attached to the current session, if any.
//!
//! ```
//! use actix_web::*;
//! use actix_identity::{Identity, IdentityMiddleware};
//!
//! #[get("/")]
//! async fn index(id: Identity) -> String {
//!     // access request identity
//!     if let Some(id) = id.id() {
//!         format!("Welcome! {}", id)
//!     } else {
//!         "Welcome Anonymous!".to_owned()
//!     }
//! }
//!
//! #[post("/login")]
//! async fn login(id: Identity) -> HttpResponse {
//!     // remember identity
//!     id.remember("User1".to_owned());
//!     HttpResponse::Ok().finish()
//! }
//!
//! #[post("/logout")]
//! async fn logout(id: Identity) -> HttpResponse {
//!     // remove identity
//!     id.forget();
//!     HttpResponse::Ok().finish()
//! }
//!
//! HttpServer::new(move || {
//!     // create cookie identity backend (inside closure, since policy is not Clone)
//!     let policy = CookieIdentityPolicy::new(&[0; 32])
//!         .name("auth-cookie")
//!         .secure(false);
//!
//!     App::new()
//!         // wrap policy into middleware identity middleware
//!         .wrap(IdentityMiddleware::new(policy))
//!         .service(services![index, login, logout])
//! })
//! # ;
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
