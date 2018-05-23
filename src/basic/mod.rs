use std::string;
use std::convert::From;

use base64;
use actix_web::{HttpRequest, HttpMessage, HttpResponse, FromRequest, ResponseError};
use actix_web::http::header;

mod config;

use errors::Error;
pub use self::config::Config;

/// Extractor for `Authorization: Basic {payload}` HTTP request header.
///
/// If header is not present or malformed, `HTTP 401` response will be returned.
/// See [Config](./struct.Config.html) struct also.
///
/// # Example
///
/// As a handler-level extractor:
///
/// ```rust
/// use actix_web_httpauth::basic::BasicAuth;
///
/// pub fn handler(auth: BasicAuth) -> String {
///     format!("Hello, {}", auth.username)
/// }
/// ```
///
/// See `examples/basic.rs` file in sources
#[derive(Debug, PartialEq)]
pub struct BasicAuth {
    pub username: String,
    pub password: String,
}

impl BasicAuth {
    pub fn error_response(cfg: &Config) -> HttpResponse {
        Error::new(cfg.clone()).error_response()
    }

    fn parse<S>(req: &HttpRequest<S>) -> Result<Self, ParseError> {
        let header = req.headers().get(header::AUTHORIZATION)
            .ok_or(ParseError)?
            .to_str()?;
        let mut parts = header.splitn(2, ' ');

        // Authorization mechanism
        match parts.next() {
            Some(mechanism) if mechanism == "Basic" => (),
            _ => return Err(ParseError),
        }

        // Authorization payload
        let payload = parts.next().ok_or(ParseError)?;
        let payload = base64::decode(payload)?;
        let payload = String::from_utf8(payload)?;
        let mut parts = payload.splitn(2, ':');
        let user = parts.next().ok_or(ParseError)?;
        let password = parts.next().ok_or(ParseError)?;

        Ok(BasicAuth{
            username: user.to_string(),
            password: password.to_string(),
        })
    }
}


impl<S> FromRequest<S> for BasicAuth {
    type Config = Config;
    type Result = Result<Self, Error>;

    fn from_request(req: &HttpRequest<S>, cfg: &<Self as FromRequest<S>>::Config) -> <Self as FromRequest<S>>::Result {
        BasicAuth::parse(req).map_err(|_| Error::new(cfg.clone()))
    }
}

#[derive(Debug)]
struct ParseError;

impl From<base64::DecodeError> for ParseError {
    fn from(_: base64::DecodeError) -> Self {
        Self{}
    }
}

impl From<header::ToStrError> for ParseError {
    fn from(_: header::ToStrError) -> Self {
        Self{}
    }
}

impl From<string::FromUtf8Error> for ParseError {
    fn from(_: string::FromUtf8Error) -> Self {
        Self{}
    }
}

#[cfg(test)]
mod tests;
