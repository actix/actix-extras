use std::default::Default;

use actix_web::{HttpRequest, FromRequest};
use actix_web::http::header::Header;

use headers::authorization::{Authorization, Basic};
use headers::www_authenticate::basic::Basic as Challenge;
use super::errors::AuthenticationError;
use super::config::ExtractorConfig;

/// [`BasicAuth`](./struct.BasicAuth.html) extractor configuration,
/// used for `WWW-Authenticate` header later.
#[derive(Debug, Clone)]
pub struct Config(Challenge);

impl Config {
    /// Set challenge `realm` attribute.
    ///
    /// The "realm" attribute indicates the scope of protection in the manner described in HTTP/1.1
    /// [RFC2617](https://tools.ietf.org/html/rfc2617#section-1.2).
    pub fn realm<T: Into<String>>(&mut self, value: T) -> &mut Config {
        self.0.realm = Some(value.into());
        self
    }
}

impl ExtractorConfig for Config {
    type Inner = Challenge;

    fn into_inner(self) -> Self::Inner {
        self.0
    }
}

impl Default for Config {
    fn default() -> Self {
        Config(Challenge::default())
    }
}

/// Extractor for HTTP Basic auth
///
/// # Example
///
/// ```rust
/// # extern crate actix_web;
/// # extern crate actix_web_httpauth;
/// use actix_web::Result;
/// use actix_web_httpauth::extractors::basic::BasicAuth;
///
/// fn index(auth: BasicAuth) -> Result<String> {
///    Ok(format!("Hello, {}!", auth.username()))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct BasicAuth(Basic);

impl BasicAuth {
    pub fn username(&self) -> &str {
        self.0.username.as_str()
    }

    pub fn password(&self) -> Option<&str> {
        match self.0.password {
            None => None,
            Some(ref pwd) => Some(pwd.as_str())
        }
    }
}

impl<S> FromRequest<S> for BasicAuth {
    type Config = Config;
    type Result = Result<Self, AuthenticationError<Challenge>>;

    fn from_request(req: &HttpRequest<S>, cfg: &<Self as FromRequest<S>>::Config) -> <Self as FromRequest<S>>::Result {
        Authorization::<Basic>::parse(req)
            .map(|auth| BasicAuth(auth.into_inner()))
            .map_err(|_| AuthenticationError::new(cfg.0.clone()))
    }
}
