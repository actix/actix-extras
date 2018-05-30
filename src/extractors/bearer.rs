use std::default::Default;

use actix_web::{HttpRequest, FromRequest};
use actix_web::http::header::Header;

use headers::authorization;
use headers::www_authenticate::bearer;
pub use headers::www_authenticate::bearer::Error;
use super::errors::AuthenticationError;
use super::config::ExtractorConfig;

/// [BearerAuth](./struct/BearerAuth.html) extractor configuration.
#[derive(Debug, Clone)]
pub struct Config(bearer::Bearer);

impl Config {
    /// Set challenge `scope` attribute.
    ///
    /// The `"scope"` attribute is a space-delimited list of case-sensitive scope values
    /// indicating the required scope of the access token for accessing the requested resource.
    pub fn scope<T: Into<String>>(&mut self, value: T) -> &mut Config {
        self.0.scope = Some(value.into());
        self
    }

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
    type Inner = bearer::Bearer;

    fn into_inner(self) -> Self::Inner {
        self.0
    }
}

impl Default for Config {
    fn default() -> Self {
        Config(bearer::Bearer::default())
    }
}

/// Extractor for HTTP Bearer auth
///
/// # Example
///
/// ```rust
/// # extern crate actix_web;
/// # extern crate actix_web_httpauth;
/// use actix_web::Result;
/// use actix_web_httpauth::extractors::bearer::BearerAuth;
///
/// fn index(auth: BearerAuth) -> Result<String> {
///    Ok(format!("Hello, user with token {}!", auth.token()))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct BearerAuth(authorization::Bearer);

impl BearerAuth {
    pub fn token(&self) -> &str {
        self.0.token.as_str()
    }
}

impl<S> FromRequest<S> for BearerAuth {
    type Config = Config;
    type Result = Result<Self, AuthenticationError<bearer::Bearer>>;

    fn from_request(req: &HttpRequest<S>, cfg: &<Self as FromRequest<S>>::Config) -> <Self as FromRequest<S>>::Result {
        authorization::Authorization::<authorization::Bearer>::parse(req)
            .map(|auth| BearerAuth(auth.into_inner()))
            .map_err(|_| AuthenticationError::new(cfg.0.clone()))
    }
}

/// Extended error customization for HTTP `Bearer` auth.
impl AuthenticationError<bearer::Bearer> {
    pub fn with_error(mut self, kind: Error) -> Self {
        *self.status_code_mut() = kind.status_code();
        self.challenge_mut().error = Some(kind);
        self
    }

    pub fn with_error_description<T: Into<String>>(mut self, desc: T) -> Self {
        self.challenge_mut().error_description = Some(desc.into());
        self
    }

    pub fn with_error_uri<T: Into<String>>(mut self, uri: T) -> Self {
        self.challenge_mut().error_uri = Some(uri.into());
        self
    }
}
