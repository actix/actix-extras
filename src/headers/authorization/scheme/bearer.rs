use std::fmt;

use bytes::{BufMut, BytesMut};
use actix_web::http::header::{HeaderValue, IntoHeaderValue, InvalidHeaderValueBytes};

use headers::authorization::scheme::Scheme;
use headers::authorization::errors::ParseError;

/// Credentials for `Bearer` authentication scheme, defined in [RFC6750](https://tools.ietf.org/html/rfc6750)
///
/// Should be used in combination with [`Authorization`](./struct.Authorization.html) header.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Bearer {
    pub token: String,
}

impl Scheme for Bearer {
    fn parse(header: &HeaderValue) -> Result<Self, ParseError> {
        // "Bearer *" length
        if header.len() < 8 {
            return Err(ParseError::Invalid);
        }

        let mut parts = header.to_str()?.splitn(2, ' ');
        match parts.next() {
            Some(scheme) if scheme == "Bearer" => (),
            _ => return Err(ParseError::MissingScheme),
        }

        let token = parts.next().ok_or(ParseError::Invalid)?;

        Ok(Bearer{
            token: token.to_string(),
        })
    }
}

impl fmt::Debug for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("Bearer ******"))
    }
}

impl fmt::Display for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("Bearer {}", self.token))
    }
}

impl IntoHeaderValue for Bearer {
    type Error = InvalidHeaderValueBytes;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        let mut buffer = BytesMut::with_capacity(7 + self.token.len());
        buffer.put("Bearer ");
        buffer.put(self.token);

        HeaderValue::from_shared(buffer.freeze())
    }
}

#[cfg(test)]
mod tests {
    use actix_web::http::header::{HeaderValue, IntoHeaderValue};
    use super::{Scheme, Bearer};

    #[test]
    fn test_parse_header() {
        let value = HeaderValue::from_static("Bearer mF_9.B5f-4.1JqM");
        let scheme = Bearer::parse(&value);

        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.token, "mF_9.B5f-4.1JqM");
    }

    #[test]
    fn test_empty_header() {
        let value = HeaderValue::from_static("");
        let scheme = Bearer::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_wrong_scheme() {
        let value = HeaderValue::from_static("OAuthToken foo");
        let scheme = Bearer::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_missing_token() {
        let value = HeaderValue::from_static("Bearer ");
        let scheme = Bearer::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_into_header_value() {
        let bearer = Bearer {
            token: "mF_9.B5f-4.1JqM".to_string(),
        };

        let result = bearer.try_into();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), HeaderValue::from_static("Bearer mF_9.B5f-4.1JqM"));
    }
}
