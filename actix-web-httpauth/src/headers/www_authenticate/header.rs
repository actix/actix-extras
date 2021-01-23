use actix_web::http::header::{
    HeaderName, HeaderValue, IntoHeaderPair, IntoHeaderValue, WWW_AUTHENTICATE,
};

use super::Challenge;

/// `WWW-Authenticate` header, described in [RFC 7235](https://tools.ietf.org/html/rfc7235#section-4.1)
///
/// This header is generic over [Challenge](./trait.Challenge.html) trait,
/// see [Basic](./basic/struct.Basic.html) and
/// [Bearer](./bearer/struct.Bearer.html) challenges for details.
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Clone)]
pub struct WwwAuthenticate<C: Challenge>(pub C);

impl<C: Challenge> IntoHeaderPair for WwwAuthenticate<C> {
    type Error = <C as IntoHeaderValue>::Error;

    fn try_into_header_pair(self) -> Result<(HeaderName, HeaderValue), Self::Error> {
        self.0.try_into_value().map(|v| (WWW_AUTHENTICATE, v))
    }
}
