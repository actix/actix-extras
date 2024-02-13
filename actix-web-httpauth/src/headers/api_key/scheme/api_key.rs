use std::{borrow::Cow, fmt, str};

use actix_web::http::header::{HeaderValue, InvalidHeaderValue, TryIntoHeaderValue};

use crate::headers::api_key::Scheme;
use crate::headers::errors::ParseError;

/// Credentials for `Basic` authentication scheme, defined in [RFC 7617](https://tools.ietf.org/html/rfc7617)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct APIKey {
    api_key: Cow<'static, str>,
}

impl APIKey {
    /// Creates `Basic` credentials with provided `user_id` and optional
    /// `password`.
    ///
    /// # Examples
    /// ```
    /// # use actix_web_httpauth::headers::authorization::Basic;
    /// let credentials = Basic::new("Alladin", Some("open sesame"));
    /// ```
    pub fn new<U>(api_key: U) -> APIKey
    where
        U: Into<Cow<'static, str>>,
    {
        APIKey {
            api_key: api_key.into(),
        }
    }

    /// Returns client's user-ID.
    pub fn api_key(&self) -> &str {
        &self.api_key.as_ref()
    }
}

impl Scheme for APIKey {
    fn parse(header: &HeaderValue) -> Result<Self, ParseError> {
        // "Basic *" length
        if header.len() < 36 {
            return Err(ParseError::Invalid);
        }
        let api_key = header.to_str()?.to_string();

        Ok(APIKey::new(api_key))
    }
}

impl fmt::Debug for APIKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("APIKey: ******"))
    }
}

impl fmt::Display for APIKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("APIKey: ******"))
    }
}

impl TryIntoHeaderValue for APIKey {
    type Error = InvalidHeaderValue;

    fn try_into_value(self) -> Result<HeaderValue, Self::Error> {
        let value = String::from(self.api_key);
        HeaderValue::from_maybe_shared(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let key = "0451f2f1-74a7-4b8c-994d-2f67675ba07c";
        let value = HeaderValue::from_static(key);
        let scheme = APIKey::parse(&value);

        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.api_key, key);
    }

    #[test]
    fn test_empty_header() {
        let value = HeaderValue::from_static("");
        let scheme = APIKey::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_wrong_scheme() {
        let value = HeaderValue::from_static("THOUSHALLNOTPASS please?");
        let scheme = APIKey::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_into_header_value() {
        let key = "0451f2f1-74a7-4b8c-994d-2f67675ba07c";
        let basic = APIKey {
            api_key: key.into(),
        };

        let result = basic.try_into_value();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), HeaderValue::from_static(key));
    }
}
