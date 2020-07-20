use std::convert::From;
use std::error::Error;
use std::fmt;
use std::str;

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
    /// Unable to convert header into the str
    ToStrError(header::ToStrError),
    /// Malformed base64 string
    Base64DecodeError(base64::DecodeError),
    /// Malformed UTF-8 string
    Utf8Error(str::Utf8Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display = match self {
            ParseError::Invalid => "Invalid header value".to_string(),
            ParseError::MissingScheme => "Missing authorization scheme".to_string(),
            ParseError::MissingField(_) => "Missing header field".to_string(),
            ParseError::ToStrError(e) => e.to_string(),
            ParseError::Base64DecodeError(e) => e.to_string(),
            ParseError::Utf8Error(e) => e.to_string(),
        };
        f.write_str(&display)
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
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
