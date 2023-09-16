//! Extractor for the "Basic" HTTP Authentication Scheme.

use std::borrow::Cow;

use actix_utils::future::{ready, Ready};
use actix_web::{dev::Payload, http::header::Header, FromRequest, HttpRequest};

use super::{config::AuthExtractorConfig, errors::AuthenticationError};
use crate::headers::{
    authorization::{Authorization, Basic},
    www_authenticate::basic::Basic as Challenge,
};

/// [`BasicAuth`] extractor configuration used for [`WWW-Authenticate`] header later.
///
/// [`WWW-Authenticate`]: crate::headers::www_authenticate::WwwAuthenticate
#[derive(Debug, Clone, Default)]
pub struct Config(Challenge);

impl Config {
    /// Set challenge `realm` attribute.
    ///
    /// The "realm" attribute indicates the scope of protection in the manner described in HTTP/1.1
    /// [RFC 2617 §1.2](https://tools.ietf.org/html/rfc2617#section-1.2).
    pub fn realm<T>(mut self, value: T) -> Config
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.realm = Some(value.into());
        self
    }
}

impl AsRef<Challenge> for Config {
    fn as_ref(&self) -> &Challenge {
        &self.0
    }
}

impl AuthExtractorConfig for Config {
    type Inner = Challenge;

    fn into_inner(self) -> Self::Inner {
        self.0
    }
}

/// Extractor for HTTP Basic auth.
///
/// # Examples
/// ```
/// use actix_web_httpauth::extractors::basic::BasicAuth;
///
/// async fn index(auth: BasicAuth) -> String {
///     format!("Hello, {}!", auth.user_id())
/// }
/// ```
///
/// If authentication fails, this extractor fetches the [`Config`] instance from the [app data] in
/// order to properly form the `WWW-Authenticate` response header.
///
/// # Examples
/// ```
/// use actix_web::{web, App};
/// use actix_web_httpauth::extractors::basic::{self, BasicAuth};
///
/// async fn index(auth: BasicAuth) -> String {
///     format!("Hello, {}!", auth.user_id())
/// }
///
/// App::new()
///     .app_data(basic::Config::default().realm("Restricted area"))
///     .service(web::resource("/index.html").route(web::get().to(index)));
/// ```
///
/// [app data]: https://docs.rs/actix-web/4/actix_web/struct.App.html#method.app_data
#[derive(Debug, Clone)]
pub struct BasicAuth(Basic);

impl BasicAuth {
    /// Returns client's user-ID.
    pub fn user_id(&self) -> &str {
        self.0.user_id()
    }

    /// Returns client's password.
    pub fn password(&self) -> Option<&str> {
        self.0.password()
    }
}

impl From<Basic> for BasicAuth {
    fn from(basic: Basic) -> Self {
        Self(basic)
    }
}

impl FromRequest for BasicAuth {
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = AuthenticationError<Challenge>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> <Self as FromRequest>::Future {
        ready(
            Authorization::<Basic>::parse(req)
                .map(|auth| BasicAuth(auth.into_scheme()))
                .map_err(|err| {
                    log::debug!("`BasicAuth` extract error: {}", err);

                    let challenge = req
                        .app_data::<Config>()
                        .map(|config| config.0.clone())
                        .unwrap_or_default();

                    AuthenticationError::new(challenge)
                }),
        )
    }
}
