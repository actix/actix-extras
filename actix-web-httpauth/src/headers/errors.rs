use std::{convert::From, error::Error, fmt, str};

use actix_web::http::header;

/// Possible errors while parsing `Authorization` header.
///
/// Should not be used directly unless you are implementing your own
/// [authentication scheme](super::Scheme).
#[derive(Debug)]
pub enum ParseError {
    /// Header value is malformed.
    Invalid,

    /// Authentication scheme is missing.
    MissingScheme,

    /// Required authentication field is missing.
    MissingField(&'static str),

    /// Unable to convert header into the str.
    ToStrError(header::ToStrError),

    /// Malformed base64 string.
    Base64DecodeError(base64::DecodeError),

    /// Malformed UTF-8 string.
    Utf8Error(str::Utf8Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Invalid => f.write_str("Invalid header value"),
            ParseError::MissingScheme => f.write_str("Missing authorization scheme"),
            ParseError::MissingField(field) => write!(f, "Missing header field ({field})"),
            ParseError::ToStrError(err) => fmt::Display::fmt(err, f),
            ParseError::Base64DecodeError(err) => fmt::Display::fmt(err, f),
            ParseError::Utf8Error(err) => fmt::Display::fmt(err, f),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::Invalid => None,
            ParseError::MissingScheme => None,
            ParseError::MissingField(_) => None,
            ParseError::ToStrError(err) => Some(err),
            ParseError::Base64DecodeError(err) => Some(err),
            ParseError::Utf8Error(err) => Some(err),
        }
    }
}

impl From<header::ToStrError> for ParseError {
    fn from(err: header::ToStrError) -> Self {
        ParseError::ToStrError(err)
    }
}

impl From<base64::DecodeError> for ParseError {
    fn from(err: base64::DecodeError) -> Self {
        ParseError::Base64DecodeError(err)
    }
}

impl From<str::Utf8Error> for ParseError {
    fn from(err: str::Utf8Error) -> Self {
        ParseError::Utf8Error(err)
    }
}
