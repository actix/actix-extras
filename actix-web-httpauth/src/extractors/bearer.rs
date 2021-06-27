//! Extractor for the "Bearer" HTTP Authentication Scheme

use std::borrow::Cow;
use std::default::Default;

use actix_web::dev::{Payload, ServiceRequest};
use actix_web::http::header::Header;
use actix_web::{FromRequest, HttpRequest};
use futures_util::future::{ready, Ready};

use super::config::AuthExtractorConfig;
use super::errors::AuthenticationError;
use super::AuthExtractor;
use crate::headers::authorization;
use crate::headers::www_authenticate::bearer;
pub use crate::headers::www_authenticate::bearer::Error;

/// [BearerAuth](./struct/BearerAuth.html) extractor configuration.
#[derive(Debug, Clone, Default)]
pub struct Config(bearer::Bearer);

impl Config {
    /// Set challenge `scope` attribute.
    ///
    /// The `"scope"` attribute is a space-delimited list of case-sensitive
    /// scope values indicating the required scope of the access token for
    /// accessing the requested resource.
    pub fn scope<T: Into<Cow<'static, str>>>(mut self, value: T) -> Config {
        self.0.scope = Some(value.into());
        self
    }

    /// Set challenge `realm` attribute.
    ///
    /// The "realm" attribute indicates the scope of protection in the manner
    /// described in HTTP/1.1 [RFC2617](https://tools.ietf.org/html/rfc2617#section-1.2).
    pub fn realm<T: Into<Cow<'static, str>>>(mut self, value: T) -> Config {
        self.0.realm = Some(value.into());
        self
    }
}

impl AsRef<bearer::Bearer> for Config {
    fn as_ref(&self) -> &bearer::Bearer {
        &self.0
    }
}

impl AuthExtractorConfig for Config {
    type Inner = bearer::Bearer;

    fn into_inner(self) -> Self::Inner {
        self.0
    }
}

// Needs `fn main` to display complete example.
#[allow(clippy::needless_doctest_main)]
/// Extractor for HTTP Bearer auth
///
/// # Example
///
/// ```
/// use actix_web_httpauth::extractors::bearer::BearerAuth;
///
/// async fn index(auth: BearerAuth) -> String {
///     format!("Hello, user with token {}!", auth.token())
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
/// use actix_web_httpauth::extractors::bearer::{BearerAuth, Config};
///
/// async fn index(auth: BearerAuth) -> String {
///     format!("Hello, {}!", auth.token())
/// }
///
/// fn main() {
///     let app = App::new()
///         .app_data(
///             Config::default()
///                 .realm("Restricted area")
///                 .scope("email photo"),
///         )
///         .service(web::resource("/index.html").route(web::get().to(index)));
/// }
/// ```
#[derive(Debug, Clone)]
pub struct BearerAuth(authorization::Bearer);

impl BearerAuth {
    /// Returns bearer token provided by client.
    pub fn token(&self) -> &str {
        self.0.token()
    }
}

impl FromRequest for BearerAuth {
    type Config = Config;
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = AuthenticationError<bearer::Bearer>;

    fn from_request(
        req: &HttpRequest,
        _payload: &mut Payload,
    ) -> <Self as FromRequest>::Future {
        ready(
            authorization::Authorization::<authorization::Bearer>::parse(req)
                .map(|auth| BearerAuth(auth.into_scheme()))
                .map_err(|_| {
                    let bearer = req
                        .app_data::<Self::Config>()
                        .map(|config| config.0.clone())
                        .unwrap_or_else(Default::default);

                    AuthenticationError::new(bearer)
                }),
        )
    }
}

impl AuthExtractor for BearerAuth {
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = AuthenticationError<bearer::Bearer>;

    fn from_service_request(req: &ServiceRequest) -> Self::Future {
        ready(
            authorization::Authorization::<authorization::Bearer>::parse(req)
                .map(|auth| BearerAuth(auth.into_scheme()))
                .map_err(|_| {
                    let bearer = req
                        .app_data::<Config>()
                        .map(|config| config.0.clone())
                        .unwrap_or_else(Default::default);

                    AuthenticationError::new(bearer)
                }),
        )
    }
}

/// Extended error customization for HTTP `Bearer` auth.
impl AuthenticationError<bearer::Bearer> {
    /// Attach `Error` to the current Authentication error.
    ///
    /// Error status code will be changed to the one provided by the `kind`
    /// Error.
    pub fn with_error(mut self, kind: Error) -> Self {
        *self.status_code_mut() = kind.status_code();
        self.challenge_mut().error = Some(kind);
        self
    }

    /// Attach error description to the current Authentication error.
    pub fn with_error_description<T>(mut self, desc: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.challenge_mut().error_description = Some(desc.into());
        self
    }

    /// Attach error URI to the current Authentication error.
    ///
    /// It is up to implementor to provide properly formed absolute URI.
    pub fn with_error_uri<T>(mut self, uri: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.challenge_mut().error_uri = Some(uri.into());
        self
    }
}
