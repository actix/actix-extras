use std::ops;
use std::fmt;

use actix_web::{HttpMessage};
use actix_web::error::ParseError;
use actix_web::http::header::{Header, HeaderName, HeaderValue, IntoHeaderValue, AUTHORIZATION};

use headers::authorization::scheme::Scheme;


/// `Authorization` header, defined in [RFC 7235](https://tools.ietf.org/html/rfc7235#section-4.2)
///
/// The "Authorization" header field allows a user agent to authenticate
/// itself with an origin server -- usually, but not necessarily, after
/// receiving a 401 (Unauthorized) response.  Its value consists of
/// credentials containing the authentication information of the user
/// agent for the realm of the resource being requested.
///
/// `Authorization` header is generic over [authentication scheme](./trait.Scheme.html).
///
/// # Example
///
/// ```rust
/// # extern crate actix_web;
/// # extern crate actix_web_httpauth;
///
/// use actix_web::{HttpRequest, Result};
/// use actix_web::http::header::Header;
/// use actix_web_httpauth::headers::authorization::{Authorization, Basic};
///
/// fn handler(req: HttpRequest) -> Result<String> {
///     let auth = Authorization::<Basic>::parse(&req)?;
///
///     Ok(format!("Hello, {}!", auth.username))
/// }
/// ```
pub struct Authorization<S: Scheme>(S);

impl<S: Scheme> Authorization<S> {
    pub fn into_inner(self) -> S {
        self.0
    }
}

impl<S: Scheme> Header for Authorization<S> {
    #[inline]
    fn name() -> HeaderName {
        AUTHORIZATION
    }

    fn parse<T: HttpMessage>(msg: &T) -> Result<Self, ParseError> {
        let header = msg.headers().get(AUTHORIZATION).ok_or(ParseError::Header)?;
        let scheme = S::parse(header).map_err(|_| ParseError::Header)?;

        Ok(Authorization(scheme))
    }
}

impl<S: Scheme> IntoHeaderValue for Authorization<S> {
    type Error = <S as IntoHeaderValue>::Error;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        self.0.try_into()
    }
}

impl<S: Scheme> fmt::Display for Authorization<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<S: Scheme> ops::Deref for Authorization<S> {
    type Target = S;

    fn deref(&self) -> &<Self as ops::Deref>::Target {
        &self.0
    }
}

impl<S: Scheme> ops::DerefMut for Authorization<S> {
    fn deref_mut(&mut self) -> &mut <Self as ops::Deref>::Target {
        &mut self.0
    }
}
