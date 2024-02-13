use std::{borrow::Cow, fmt, str};

use actix_web::{
    http::header::{HeaderValue, InvalidHeaderValue, TryIntoHeaderValue},
    web::{BufMut, BytesMut},
};
use base64::{prelude::BASE64_STANDARD, Engine};

use crate::headers::authorization::Scheme;
use crate::headers::errors::ParseError;

/// Credentials for `Basic` authentication scheme, defined in [RFC 7617](https://tools.ietf.org/html/rfc7617)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Basic {
    user_id: Cow<'static, str>,
    password: Option<Cow<'static, str>>,
}

impl Basic {
    /// Creates `Basic` credentials with provided `user_id` and optional
    /// `password`.
    ///
    /// # Examples
    /// ```
    /// # use actix_web_httpauth::headers::authorization::Basic;
    /// let credentials = Basic::new("Alladin", Some("open sesame"));
    /// ```
    pub fn new<U, P>(user_id: U, password: Option<P>) -> Basic
    where
        U: Into<Cow<'static, str>>,
        P: Into<Cow<'static, str>>,
    {
        Basic {
            user_id: user_id.into(),
            password: password.map(Into::into),
        }
    }

    /// Returns client's user-ID.
    pub fn user_id(&self) -> &str {
        self.user_id.as_ref()
    }

    /// Returns client's password if provided.
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl Scheme for Basic {
    fn parse(header: &HeaderValue) -> Result<Self, ParseError> {
        // "Basic *" length
        if header.len() < 7 {
            return Err(ParseError::Invalid);
        }

        let mut parts = header.to_str()?.splitn(2, ' ');
        match parts.next() {
            Some("Basic") => (),
            _ => return Err(ParseError::MissingScheme),
        }

        let decoded = BASE64_STANDARD.decode(parts.next().ok_or(ParseError::Invalid)?)?;
        let mut credentials = str::from_utf8(&decoded)?.splitn(2, ':');

        let user_id = credentials
            .next()
            .ok_or(ParseError::MissingField("user_id"))
            .map(|user_id| user_id.to_string().into())?;

        let password = credentials
            .next()
            .ok_or(ParseError::MissingField("password"))
            .map(|password| {
                if password.is_empty() {
                    None
                } else {
                    Some(password.to_string().into())
                }
            })?;

        Ok(Basic { user_id, password })
    }
}

impl fmt::Debug for Basic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Basic {}:******", self.user_id))
    }
}

impl fmt::Display for Basic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Basic {}:******", self.user_id))
    }
}

impl TryIntoHeaderValue for Basic {
    type Error = InvalidHeaderValue;

    fn try_into_value(self) -> Result<HeaderValue, Self::Error> {
        let credential_length =
            self.user_id.len() + 1 + self.password.as_ref().map_or(0, |pwd| pwd.len());
        // The length of BASE64 encoded bytes is `4 * credential_length.div_ceil(3)`
        // TODO: Use credential_length.div_ceil(3) when `int_roundings` becomes stable
        // https://github.com/rust-lang/rust/issues/88581
        let mut value = String::with_capacity(6 + 4 * (credential_length + 2) / 3);
        let mut credentials = BytesMut::with_capacity(credential_length);

        credentials.extend_from_slice(self.user_id.as_bytes());
        credentials.put_u8(b':');
        if let Some(ref password) = self.password {
            credentials.extend_from_slice(password.as_bytes());
        }

        value.push_str("Basic ");
        BASE64_STANDARD.encode_string(&credentials, &mut value);

        HeaderValue::from_maybe_shared(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let value = HeaderValue::from_static("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.user_id, "Aladdin");
        assert_eq!(scheme.password, Some("open sesame".into()));
    }

    #[test]
    fn test_empty_password() {
        let value = HeaderValue::from_static("Basic QWxhZGRpbjo=");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.user_id, "Aladdin");
        assert_eq!(scheme.password, None);
    }

    #[test]
    fn test_empty_header() {
        let value = HeaderValue::from_static("");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_wrong_scheme() {
        let value = HeaderValue::from_static("THOUSHALLNOTPASS please?");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_missing_credentials() {
        let value = HeaderValue::from_static("Basic ");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_missing_credentials_colon() {
        let value = HeaderValue::from_static("Basic QWxsYWRpbg==");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_into_header_value() {
        let basic = Basic {
            user_id: "Aladdin".into(),
            password: Some("open sesame".into()),
        };

        let result = basic.try_into_value();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            HeaderValue::from_static("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==")
        );
    }
}
