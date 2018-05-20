use std::fmt;
use std::error;
use std::string;

use base64;
use actix_web::HttpResponse;
use actix_web::error::ResponseError;
use actix_web::http::{StatusCode, header};

#[derive(Debug, PartialEq)]
pub enum Error {
    HeaderMissing,  // HTTP 401
    // TODO: Ensure that 401 should be returned if not a `Basic` mechanism is received
    InvalidMechanism,  // HTTP 401 ?
    HeaderMalformed,  // HTTP 400
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            Error::HeaderMissing => "HTTP 'Authorization' header is missing",
            Error::InvalidMechanism => "Wrong mechanizm for a HTTP 'Authorization' header, expected 'Basic'",
            Error::HeaderMalformed => "Malformed HTTP 'Authorization' header",
        };

        f.write_str(msg)
    }
}

impl error::Error for Error {}

impl From<header::ToStrError> for Error {
    fn from(_: header::ToStrError) -> Self {
        Error::HeaderMalformed
    }
}

impl From<base64::DecodeError> for Error {
    fn from(_: base64::DecodeError) -> Self {
        Error::HeaderMalformed
    }
}

impl From<string::FromUtf8Error> for Error {
    fn from(_: string::FromUtf8Error) -> Self {
        Error::HeaderMalformed
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        let status = match self {
            Error::HeaderMissing => StatusCode::UNAUTHORIZED,
            Error::InvalidMechanism => StatusCode::UNAUTHORIZED,
            Error::HeaderMalformed => StatusCode::BAD_REQUEST,
        };

        HttpResponse::build(status)
            .header("WWW-Authenticate", "Basic, charset=\"UTF-8\"")
            .finish()
    }

}
