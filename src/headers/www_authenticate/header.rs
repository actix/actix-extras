use actix_web::{HttpMessage};
use actix_web::error::ParseError;
use actix_web::http::header::{Header, HeaderName, HeaderValue, IntoHeaderValue, WWW_AUTHENTICATE};

use super::Challenge;

/// `WWW-Authenticate` header, described in [RFC 7235](https://tools.ietf.org/html/rfc7235#section-4.1)
///
/// `WWW-Authenticate` header is generic over [Challenge](./trait.Challenge.html)
///
/// # Example
///
/// ```rust
/// # extern crate actix_web;
/// # extern crate actix_web_httpauth;
///
/// use actix_web::{HttpRequest, HttpResponse};
/// use actix_web::http::StatusCode;
/// use actix_web_httpauth::headers::www_authenticate::{WWWAuthenticate};
/// use actix_web_httpauth::headers::www_authenticate::basic::Basic;
///
/// fn handler(req: HttpRequest) -> HttpResponse {
///     let challenge = Basic {
///         realm: Some("Restricted area".to_string()),
///     };
///     req.build_response(StatusCode::UNAUTHORIZED)
///         .set(WWWAuthenticate(challenge))
///         .finish()
/// }
/// ```
pub struct WWWAuthenticate<C: Challenge>(pub C);

impl<C: Challenge> Header for WWWAuthenticate<C> {
    fn name() -> HeaderName {
        WWW_AUTHENTICATE
    }

    fn parse<T: HttpMessage>(_msg: &T) -> Result<Self, ParseError> {
        unimplemented!()
    }
}

impl<C: Challenge> IntoHeaderValue for WWWAuthenticate<C> {
    type Error = <C as IntoHeaderValue>::Error;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        self.0.try_into()
    }
}
