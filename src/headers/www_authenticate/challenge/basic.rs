//! Challenge for the "Basic" HTTP Authentication Scheme

use std::borrow::Cow;
use std::default::Default;
use std::fmt;
use std::str;

use actix_web::http::header::{
    HeaderValue, IntoHeaderValue, InvalidHeaderValue,
};
use bytes::{BufMut, Bytes, BytesMut};

use super::Challenge;
use crate::utils;

/// Challenge for [`WWW-Authenticate`] header with HTTP Basic auth scheme,
/// described in [RFC 7617](https://tools.ietf.org/html/rfc7617)
///
/// ## Example
///
/// ```rust
/// # use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
/// use actix_web_httpauth::headers::www_authenticate::basic::Basic;
/// use actix_web_httpauth::headers::www_authenticate::WwwAuthenticate;
///
/// fn index(_req: HttpRequest) -> HttpResponse {
///     let challenge = Basic::with_realm("Restricted area");
///
///     HttpResponse::Unauthorized()
///         .set(WwwAuthenticate(challenge))
///         .finish()
/// }
/// ```
///
/// [`WWW-Authenticate`]: ../struct.WwwAuthenticate.html
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Clone)]
pub struct Basic {
    // "realm" parameter is optional now: https://tools.ietf.org/html/rfc7235#appendix-A
    pub(crate) realm: Option<Cow<'static, str>>,
}

impl Basic {
    /// Creates new `Basic` challenge with an empty `realm` field.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use actix_web_httpauth::headers::www_authenticate::basic::Basic;
    /// let challenge = Basic::new();
    /// ```
    pub fn new() -> Basic {
        Default::default()
    }

    /// Creates new `Basic` challenge from the provided `realm` field value.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # use actix_web_httpauth::headers::www_authenticate::basic::Basic;
    /// let challenge = Basic::with_realm("Restricted area");
    /// ```
    ///
    /// ```rust
    /// # use actix_web_httpauth::headers::www_authenticate::basic::Basic;
    /// let my_realm = "Earth realm".to_string();
    /// let challenge = Basic::with_realm(my_realm);
    /// ```
    pub fn with_realm<T>(value: T) -> Basic
    where
        T: Into<Cow<'static, str>>,
    {
        Basic {
            realm: Some(value.into()),
        }
    }
}

#[doc(hidden)]
impl Challenge for Basic {
    fn to_bytes(&self) -> Bytes {
        // 5 is for `"Basic"`, 9 is for `"realm=\"\""`
        let length = 5 + self.realm.as_ref().map_or(0, |realm| realm.len() + 9);
        let mut buffer = BytesMut::with_capacity(length);
        buffer.put("Basic");
        if let Some(ref realm) = self.realm {
            buffer.put(" realm=\"");
            utils::put_quoted(&mut buffer, realm);
            buffer.put("\"");
        }

        buffer.freeze()
    }
}

impl fmt::Display for Basic {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let bytes = self.to_bytes();
        let repr = str::from_utf8(&bytes)
            // Should not happen since challenges are crafted manually
            // from `&'static str`'s and Strings
            .map_err(|_| fmt::Error)?;

        f.write_str(repr)
    }
}

impl IntoHeaderValue for Basic {
    type Error = InvalidHeaderValue;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        HeaderValue::from_maybe_shared(self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::Basic;
    use actix_web::http::header::IntoHeaderValue;

    #[test]
    fn test_plain_into_header_value() {
        let challenge = Basic {
            realm: None,
        };

        let value = challenge.try_into();
        assert!(value.is_ok());
        let value = value.unwrap();
        assert_eq!(value, "Basic");
    }

    #[test]
    fn test_with_realm_into_header_value() {
        let challenge = Basic {
            realm: Some("Restricted area".into()),
        };

        let value = challenge.try_into();
        assert!(value.is_ok());
        let value = value.unwrap();
        assert_eq!(value, "Basic realm=\"Restricted area\"");
    }
}
