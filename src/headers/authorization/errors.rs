use std::str;
use std::fmt;
use std::error::Error;
use std::convert::From;

use base64;
use actix_web::http::header;

/// Possible errors while parsing `Authorization` header.
///
/// Should not be used directly unless you are implementing
/// your own [authentication scheme](./trait.Scheme.html).
#[derive(Debug)]
pub enum ParseError {
    /// Header value is malformed
    Invalid,
    /// Authentication scheme is missing
    MissingScheme,
    /// Required authentication field is missing
    MissingField(&'static str),
    ToStrError(header::ToStrError),
    Base64DecodeError(base64::DecodeError),
    Utf8Error(str::Utf8Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match self {
            ParseError::Invalid => "Invalid header value",
            ParseError::MissingScheme => "Missing authorization scheme",
            ParseError::MissingField(_) => "Missing header field",
            ParseError::ToStrError(e) => e.description(),
            ParseError::Base64DecodeError(e) => e.description(),
            ParseError::Utf8Error(e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            ParseError::Invalid => None,
            ParseError::MissingScheme => None,
            ParseError::MissingField(_) => None,
            ParseError::ToStrError(e) => Some(e),
            ParseError::Base64DecodeError(e) => Some(e),
            ParseError::Utf8Error(e) => Some(e),
        }
    }
}

impl From<header::ToStrError> for ParseError {
    fn from(e: header::ToStrError) -> Self {
        ParseError::ToStrError(e)
    }
}
impl From<base64::DecodeError> for ParseError {
    fn from(e: base64::DecodeError) -> Self {
        ParseError::Base64DecodeError(e)
    }
}
impl From<str::Utf8Error> for ParseError {
    fn from(e: str::Utf8Error) -> Self {
        ParseError::Utf8Error(e)
    }
}
