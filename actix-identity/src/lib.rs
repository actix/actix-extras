//! Opinionated request identity service for Actix Web apps.
//!
//! [`IdentityService`] middleware can be used with different policies types to store
//! identity information.
//!
//! A cookie based policy is provided. [`CookieIdentityPolicy`] uses cookies as identity storage.
//!
//! To access current request identity, use the [`Identity`] extractor.
//!
//! ```
//! use actix_web::*;
//! use actix_identity::{Identity, CookieIdentityPolicy, IdentityService};
//!
//! #[get("/")]
//! async fn index(id: Identity) -> String {
//!     // access request identity
//!     if let Some(id) = id.identity() {
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
//!         .wrap(IdentityService::new(policy))
//!         .service(services![index, login, logout])
//! })
//! # ;
//! ```

#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible)]

use std::future::Future;

use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    Error, HttpMessage, Result,
};

mod cookie;
mod identity;
mod middleware;

pub use self::cookie::CookieIdentityPolicy;
pub use self::identity::Identity;
pub use self::middleware::IdentityService;

/// Identity policy.
pub trait IdentityPolicy: Sized + 'static {
    /// The return type of the middleware
    type Future: Future<Output = Result<Option<String>, Error>>;

    /// The return type of the middleware
    type ResponseFuture: Future<Output = Result<(), Error>>;

    /// Parse the session from request and load data from a service identity.
    fn from_request(&self, req: &mut ServiceRequest) -> Self::Future;

    /// Write changes to response
    fn to_response<B>(
        &self,
        identity: Option<String>,
        changed: bool,
        response: &mut ServiceResponse<B>,
    ) -> Self::ResponseFuture;
}

/// Helper trait that allows to get Identity.
///
/// It could be used in middleware but identity policy must be set before any other middleware that
/// needs identity. RequestIdentity is implemented both for `ServiceRequest` and `HttpRequest`.
pub trait RequestIdentity {
    fn get_identity(&self) -> Option<String>;
}

impl<T> RequestIdentity for T
where
    T: HttpMessage,
{
    fn get_identity(&self) -> Option<String> {
        Identity::get_identity(&self.extensions())
    }
}

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
                .wrap(IdentityService::new(f(CookieIdentityPolicy::new(
                    &COOKIE_KEY_MASTER,
                )
                .secure(false)
                .name(COOKIE_NAME))))
                .service(web::resource("/").to(|id: Identity| async move {
                    let identity = id.identity();
                    if identity.is_none() {
                        id.remember(COOKIE_LOGIN.to_string())
                    }
                    web::Json(identity)
                })),
        )
        .await
    }
}
