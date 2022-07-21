use actix_web::{
    error::ParseError,
    http::header::{Header, HeaderName, HeaderValue, TryIntoHeaderValue, WWW_AUTHENTICATE},
    HttpMessage,
};

use super::Challenge;

/// `WWW-Authenticate` header, described in [RFC 7235].
///
/// This header is generic over the [`Challenge`] trait, see [`Basic`](super::basic::Basic) and
/// [`Bearer`](super::bearer::Bearer) challenges for details.
///
/// [RFC 7235]: https://tools.ietf.org/html/rfc7235#section-4.1
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WwwAuthenticate<C: Challenge>(pub C);

impl<C: Challenge> Header for WwwAuthenticate<C> {
    #[inline]
    fn name() -> HeaderName {
        WWW_AUTHENTICATE
    }

    fn parse<T: HttpMessage>(_msg: &T) -> Result<Self, ParseError> {
        unimplemented!()
    }
}

impl<C: Challenge> TryIntoHeaderValue for WwwAuthenticate<C> {
    type Error = <C as TryIntoHeaderValue>::Error;

    fn try_into_value(self) -> Result<HeaderValue, Self::Error> {
        self.0.try_into_value()
    }
}
