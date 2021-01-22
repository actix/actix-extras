// use std::marker::PhantomData;

use super::AuthenticationError;
use crate::headers::www_authenticate::Challenge;
use super::CompleteErrorResponse;
// use super::AuthExtractor;

/// Trait implemented for types that provides configuration
/// for the authentication [extractors].
///
/// [extractors]: ./trait.AuthExtractor.html
pub trait AuthExtractorConfig: 'static + std::fmt::Debug + std::clone::Clone + std::default::Default {
    /// Associated challenge type.
    type Inner: Challenge;

    /// Associated error response callback.
    type Builder: CompleteErrorResponse;

    /// Convert the config instance into a HTTP challenge.
    fn into_inner(self) -> Self::Inner;
}

impl<T> From<T> for AuthenticationError<T>
where
    T: AuthExtractorConfig,
{
    fn from(config: T) -> Self {
        AuthenticationError::new(config)
    }
}
