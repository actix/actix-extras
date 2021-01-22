//! Extractor for the "Basic" HTTP Authentication Scheme

use std::borrow::Cow;
use std::default::Default;
use std::marker::PhantomData;

use actix_web::dev::{Payload, ServiceRequest};
use actix_web::http::header::Header;
use actix_web::{FromRequest, HttpRequest};
use futures_util::future::{ready, Ready};

use super::config::AuthExtractorConfig;
use super::errors::{AuthenticationError, CompleteErrorResponse, DefaultErrorResponse};
use super::AuthExtractor;
use crate::headers::authorization::{Authorization, Basic};
use crate::headers::www_authenticate::basic::Basic as Challenge;

/// [`BasicAuth`] extractor configuration,
/// used for [`WWW-Authenticate`] header later.
///
/// [`BasicAuth`]: ./struct.BasicAuth.html
/// [`WWW-Authenticate`]:
/// ../../headers/www_authenticate/struct.WwwAuthenticate.html
#[derive(Debug, Clone, Default)]
pub struct Config<B: CompleteErrorResponse = DefaultErrorResponse>(
    Challenge,
    PhantomData<B>,
);

impl<B: CompleteErrorResponse> Config<B> {
    /// Set challenge `realm` attribute.
    ///
    /// The "realm" attribute indicates the scope of protection in the manner
    /// described in HTTP/1.1 [RFC2617](https://tools.ietf.org/html/rfc2617#section-1.2).
    pub fn realm<T>(mut self, value: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.realm = Some(value.into());
        self
    }
}

impl<B: CompleteErrorResponse> AsRef<Challenge> for Config<B> {
    fn as_ref(&self) -> &Challenge {
        &self.0
    }
}

impl<B: CompleteErrorResponse> AuthExtractorConfig for Config<B> {
    type Inner = Challenge;
    type Builder = B;

    fn into_inner(self) -> Self::Inner {
        self.0
    }
}

// Needs `fn main` to display complete example.
#[allow(clippy::needless_doctest_main)]
/// Extractor for HTTP Basic auth.
///
/// # Example
///
/// ```
/// use actix_web::Result;
/// use actix_web_httpauth::extractors::basic::BasicAuth;
///
/// async fn index(auth: BasicAuth) -> String {
///     format!("Hello, {}!", auth.user_id())
/// }
/// ```
///
/// If authentication fails, this extractor fetches the [`Config`] instance
/// from the [app data] in order to properly form the `WWW-Authenticate`
/// response header.
///
/// ## Example
///
/// ```
/// use actix_web::{web, App};
/// use actix_web_httpauth::extractors::basic::BasicAuth;
///
/// async fn index(auth: BasicAuth) -> String {
///     format!("Hello, {}!", auth.user_id())
/// }
///
/// fn main() {
///     let app = App::new()
///         .data(<BasicAuth>::default_config().realm("Restricted area"))
///         .service(web::resource("/index.html").route(web::get().to(index)));
/// }
/// ```
///
/// [`Config`]: ./struct.Config.html
/// [app data]: https://docs.rs/actix-web/1.0.0-beta.5/actix_web/struct.App.html#method.data
#[derive(Debug, Clone)]
pub struct BasicAuth<B: CompleteErrorResponse = DefaultErrorResponse>(
    Basic,
    PhantomData<B>,
);

impl<B: CompleteErrorResponse> BasicAuth<B> {
    /// Returns client's user-ID.
    pub fn user_id(&self) -> &Cow<'static, str> {
        &self.0.user_id()
    }

    /// Returns client's password.
    pub fn password(&self) -> Option<&Cow<'static, str>> {
        self.0.password()
    }

    /// Returns the default BasicAuth configuraion
    pub fn default_config() -> <Self as FromRequest>::Config {
        <Self as FromRequest>::Config::default()
    }
}

impl<B: CompleteErrorResponse> FromRequest for BasicAuth<B> {
    type Future = Ready<Result<Self, Self::Error>>;
    type Config = Config<B>;
    type Error = AuthenticationError<Self::Config>;

    fn from_request(
        req: &HttpRequest,
        _: &mut Payload,
    ) -> <Self as FromRequest>::Future {
        ready(
            Authorization::<Basic>::parse(req)
                .map(|auth| BasicAuth(auth.into_scheme(), PhantomData))
                .map_err(|_| AuthenticationError::default(req)),
        )
    }
}

impl<B: CompleteErrorResponse> AuthExtractor for BasicAuth<B> {
    fn from_service_request(req: &ServiceRequest) -> Self::Future {
        ready(
            Authorization::<Basic>::parse(req)
                .map(|auth| BasicAuth(auth.into_scheme(), PhantomData))
                .map_err(|_| AuthenticationError::default2(req)),
        )
    }
}
