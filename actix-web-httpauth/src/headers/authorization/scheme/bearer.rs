use std::{borrow::Cow, fmt};

use actix_web::{
    http::header::{HeaderValue, InvalidHeaderValue, TryIntoHeaderValue},
    web::{BufMut, BytesMut},
};

use crate::headers::authorization::scheme::Scheme;
use crate::headers::errors::ParseError;

/// Credentials for `Bearer` authentication scheme, defined in [RFC 6750].
///
/// Should be used in combination with [`Authorization`] header.
///
/// [RFC 6750]: https://tools.ietf.org/html/rfc6750
/// [`Authorization`]: crate::headers::authorization::Authorization
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Bearer {
    token: Cow<'static, str>,
}

impl Bearer {
    /// Creates new `Bearer` credentials with the token provided.
    ///
    /// # Example
    /// ```
    /// # use actix_web_httpauth::headers::authorization::Bearer;
    /// let credentials = Bearer::new("mF_9.B5f-4.1JqM");
    /// ```
    pub fn new<T>(token: T) -> Bearer
    where
        T: Into<Cow<'static, str>>,
    {
        Bearer {
            token: token.into(),
        }
    }

    /// Gets reference to the credentials token.
    pub fn token(&self) -> &str {
        self.token.as_ref()
    }
}

impl Scheme for Bearer {
    fn parse(header: &HeaderValue) -> Result<Self, ParseError> {
        // "Bearer *" length
        if header.len() < 8 {
            return Err(ParseError::Invalid);
        }

        let mut parts = header.to_str()?.splitn(2, ' ');

        match parts.next() {
            Some("Bearer") => {}
            _ => return Err(ParseError::MissingScheme),
        }

        let token = parts.next().ok_or(ParseError::Invalid)?;

        Ok(Bearer {
            token: token.to_string().into(),
        })
    }
}

impl fmt::Debug for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Bearer ******"))
    }
}

impl fmt::Display for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Bearer {}", self.token))
    }
}

impl TryIntoHeaderValue for Bearer {
    type Error = InvalidHeaderValue;

    fn try_into_value(self) -> Result<HeaderValue, Self::Error> {
        let mut buffer = BytesMut::with_capacity(7 + self.token.len());
        buffer.put(&b"Bearer "[..]);
        buffer.extend_from_slice(self.token.as_bytes());

        HeaderValue::from_maybe_shared(buffer.freeze())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let bearer = Bearer::new("mF_9.B5f-4.1JqM");

        let result = bearer.try_into_value();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            HeaderValue::from_static("Bearer mF_9.B5f-4.1JqM")
        );
    }
}
