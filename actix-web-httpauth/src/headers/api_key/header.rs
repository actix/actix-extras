use std::fmt;

use actix_web::{
    error::ParseError,
    http::header::{Header, HeaderName, HeaderValue, TryIntoHeaderValue, AUTHORIZATION},
    HttpMessage,
};

use crate::headers::api_key::scheme::Scheme;

/// `Authorization` header, defined in [RFC 7235](https://tools.ietf.org/html/rfc7235#section-4.2)
///
/// The "Authorization" header field allows a user agent to authenticate itself with an origin
/// server—usually, but not necessarily, after receiving a 401 (Unauthorized) response. Its value
/// consists of credentials containing the authentication information of the user agent for the
/// realm of the resource being requested.
///
/// `Authorization` is generic over an [authentication scheme](Scheme).
///
/// # Examples
/// ```
/// # use actix_web::{HttpRequest, Result, http::header::Header};
/// # use actix_web_httpauth::headers::authorization::{Authorization, Basic};
/// fn handler(req: HttpRequest) -> Result<String> {
///     let auth = Authorization::<Basic>::parse(&req)?;
///
///     Ok(format!("Hello, {}!", auth.as_ref().user_id()))
/// }
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct XAPIKey<S: Scheme>(S);

impl<S: Scheme> XAPIKey<S> {
    /// Consumes `X-API-KEY` header and returns inner [`Scheme`] implementation.
    pub fn into_scheme(self) -> S {
        self.0
    }
}

impl<S: Scheme> From<S> for XAPIKey<S> {
    fn from(scheme: S) -> XAPIKey<S> {
        XAPIKey(scheme)
    }
}

impl<S: Scheme> AsRef<S> for XAPIKey<S> {
    fn as_ref(&self) -> &S {
        &self.0
    }
}

impl<S: Scheme> AsMut<S> for XAPIKey<S> {
    fn as_mut(&mut self) -> &mut S {
        &mut self.0
    }
}

impl<S: Scheme> fmt::Display for XAPIKey<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<S: Scheme> Header for XAPIKey<S> {
    #[inline]
    fn name() -> HeaderName {
        HeaderName::from_static("x-api-key")
    }

    fn parse<T: HttpMessage>(msg: &T) -> Result<Self, ParseError> {
        let header = msg.headers().get(Self::name()).ok_or(ParseError::Header)?;
        let scheme = S::parse(header).map_err(|_| ParseError::Header)?;

        Ok(XAPIKey(scheme))
    }
}

impl<S: Scheme> TryIntoHeaderValue for XAPIKey<S> {
    type Error = <S as TryIntoHeaderValue>::Error;

    fn try_into_value(self) -> Result<HeaderValue, Self::Error> {
        self.0.try_into_value()
    }
}
