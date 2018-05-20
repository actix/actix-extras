use std::fmt;
use std::error::Error;
use std::string;

use base64;
use actix_web::HttpResponse;
use actix_web::error::ResponseError;
use actix_web::http::{StatusCode, header};

#[derive(Debug, PartialEq)]
pub enum AuthError {
    HeaderMissing,  // HTTP 401
    // TODO: Ensure that 401 should be returned if not a `Basic` mechanism is received
    InvalidMechanism,  // HTTP 401 ?
    HeaderMalformed,  // HTTP 400
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl Error for AuthError {
    fn description(&self) -> &str {
        match *self {
            AuthError::HeaderMissing => "HTTP 'Authorization' header is missing",
            AuthError::InvalidMechanism => "Wrong mechanism for a HTTP 'Authorization' header, expected 'Basic'",
            AuthError::HeaderMalformed => "Malformed HTTP 'Authorization' header",
        }
    }
}

impl From<header::ToStrError> for AuthError {
    fn from(_: header::ToStrError) -> Self {
        AuthError::HeaderMalformed
    }
}

impl From<base64::DecodeError> for AuthError {
    fn from(_: base64::DecodeError) -> Self {
        AuthError::HeaderMalformed
    }
}

impl From<string::FromUtf8Error> for AuthError {
    fn from(_: string::FromUtf8Error) -> Self {
        AuthError::HeaderMalformed
    }
}

impl ResponseError for AuthError {
    fn error_response(&self) -> HttpResponse {
        let status = match *self {
            AuthError::HeaderMissing => StatusCode::UNAUTHORIZED,
            AuthError::InvalidMechanism => StatusCode::UNAUTHORIZED,
            AuthError::HeaderMalformed => StatusCode::BAD_REQUEST,
        };

        HttpResponse::build(status)
            .header("WWW-Authenticate", "Basic, charset=\"UTF-8\"")
            .finish()
    }

}
