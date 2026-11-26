use std::{borrow::Cow, fmt, str};

use actix_web::{
    http::header::{HeaderValue, InvalidHeaderValue, TryIntoHeaderValue},
    web::{BufMut, Bytes, BytesMut},
};

use super::{super::Challenge, BearerBuilder, Error};
use crate::utils;

/// Challenge for [`WWW-Authenticate`] header with HTTP Bearer auth scheme, described in [RFC 6750].
///
/// # Examples
/// ```
/// # use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
/// use actix_web_httpauth::headers::www_authenticate::bearer::{
///     Bearer, Error,
/// };
/// use actix_web_httpauth::headers::www_authenticate::WwwAuthenticate;
///
/// fn index(_req: HttpRequest) -> HttpResponse {
///     let challenge = Bearer::build()
///         .realm("example")
///         .scope("openid profile email")
///         .error(Error::InvalidToken)
///         .error_description("The access token expired")
///         .error_uri("http://example.org")
///         .finish();
///
///     HttpResponse::Unauthorized()
///         .insert_header(WwwAuthenticate(challenge))
///         .finish()
/// }
/// ```
///
/// [`WWW-Authenticate`]: crate::headers::www_authenticate::WwwAuthenticate
/// [RFC 6750]: https://www.rfc-editor.org/info/rfc6750/#section-3
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bearer {
    pub(crate) scope: Option<Cow<'static, str>>,
    pub(crate) realm: Option<Cow<'static, str>>,
    pub(crate) error: Option<Error>,
    pub(crate) error_description: Option<Cow<'static, str>>,
    pub(crate) error_uri: Option<Cow<'static, str>>,
}

impl Bearer {
    /// Creates the builder for `Bearer` challenge.
    ///
    /// # Examples
    /// ```
    /// # use actix_web_httpauth::headers::www_authenticate::bearer::{Bearer};
    /// let challenge = Bearer::build()
    ///     .realm("Restricted area")
    ///     .scope("openid profile email")
    ///     .finish();
    /// ```
    pub fn build() -> BearerBuilder {
        BearerBuilder::default()
    }
}

const BEARER: &[u8] = b"Bearer";
const REALM: &[u8] = b" realm=\"";
const SCOPE: &[u8] = b" scope=\"";
const ERROR: &[u8] = b" error=\"";
const ERROR_DESCRIPTION: &[u8] = b" error_description=\"";
const ERROR_URI: &[u8] = b" error_uri=\"";
const ATTRIBUTE_CLOSE: &[u8] = b"\",";

fn opt_len(val: &Option<impl AsRef<str>>, attribute: &[u8]) -> usize {
    val.as_ref().map_or(0, |val| {
        attribute.len() + val.as_ref().len() + ATTRIBUTE_CLOSE.len()
    })
}

#[doc(hidden)]
impl Challenge for Bearer {
    fn to_bytes(&self) -> Bytes {
        let capacity = BEARER.len()
            + opt_len(&self.realm, REALM)
            + opt_len(&self.scope, SCOPE)
            + opt_len(&self.error, ERROR)
            + opt_len(&self.error_description, ERROR_DESCRIPTION)
            + opt_len(&self.error_uri, ERROR_URI);

        let mut buffer = BytesMut::with_capacity(capacity);
        buffer.put(&b"Bearer"[..]);

        if let Some(ref realm) = self.realm {
            buffer.extend_from_slice(REALM);
            utils::put_quoted(&mut buffer, realm);
            buffer.extend_from_slice(ATTRIBUTE_CLOSE);
        }

        if let Some(ref scope) = self.scope {
            buffer.extend_from_slice(SCOPE);
            utils::put_quoted(&mut buffer, scope);
            buffer.extend_from_slice(ATTRIBUTE_CLOSE);
        }

        if let Some(ref error) = self.error {
            buffer.extend_from_slice(ERROR);
            utils::put_quoted(&mut buffer, error.as_str());
            buffer.extend_from_slice(ATTRIBUTE_CLOSE);
        }

        if let Some(ref error_description) = self.error_description {
            buffer.extend_from_slice(ERROR_DESCRIPTION);
            utils::put_quoted(&mut buffer, error_description);
            buffer.extend_from_slice(ATTRIBUTE_CLOSE);
        }

        if let Some(ref error_uri) = self.error_uri {
            buffer.extend_from_slice(ERROR_URI);
            utils::put_quoted(&mut buffer, error_uri);
            buffer.extend_from_slice(ATTRIBUTE_CLOSE);
        }

        // If we wrote attributes, remove the last trailing comma
        if capacity > BEARER.len() {
            buffer.truncate(buffer.len() - 1);
        }

        buffer.freeze()
    }
}

impl fmt::Display for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let bytes = self.to_bytes();

        let repr = str::from_utf8(&bytes)
            // Should not happen since challenges are crafted manually
            // from `&'static str`'s and Strings
            .map_err(|_| fmt::Error)?;

        f.write_str(repr)
    }
}

impl TryIntoHeaderValue for Bearer {
    type Error = InvalidHeaderValue;

    fn try_into_value(self) -> Result<HeaderValue, Self::Error> {
        HeaderValue::from_maybe_shared(self.to_bytes())
    }
}
