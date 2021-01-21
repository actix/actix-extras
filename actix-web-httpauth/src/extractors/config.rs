use std::marker::PhantomData;

use super::AuthenticationError;
use crate::headers::www_authenticate::Challenge;
use super::CompleteErrorResponse;
use super::AuthExtractor;

/// Trait implemented for types that provides configuration
/// for the authentication [extractors].
///
/// [extractors]: ./trait.AuthExtractor.html
pub trait AuthExtractorConfig {
    /// Associated challenge type.
    type Inner: Challenge;

    /// Convert the config instance into a HTTP challenge.
    fn into_inner(self) -> Self::Inner;
}

impl<T, B> From<T> for AuthenticationError<<T as AuthExtractorConfig>::Inner, B>
where
    T: AuthExtractorConfig,
    B: CompleteErrorResponse,
{
    fn from(config: T) -> Self {
        AuthenticationError::new(config.into_inner())
    }
}


/// Relate AuthExtractorConfig with a CompleteErrorResponse implementation
pub struct TypedConfig<T, B>
where
    T: AuthExtractorConfig,
    B: CompleteErrorResponse,
{
    a: T,
    _p: PhantomData<B>,
}

impl<T, B> TypedConfig<T, B>
where
    T: AuthExtractorConfig,
    B: CompleteErrorResponse,
{
    /// Relate the config with response implementation of the credential
    pub fn hint<E, F, A: AuthExtractor<Error = E, Future = F, CompleteResponse = B>>(_: &A, config: T) -> Self {
        Self::from(config)
    }
}

impl<T, B> From<T> for TypedConfig<T, B>
where
    T: AuthExtractorConfig,
    B: CompleteErrorResponse,
{
    fn from(config: T) -> Self {
        Self { a: config, _p: PhantomData }
    }
}

impl<T, B> From<TypedConfig<T, B>> for AuthenticationError<<T as AuthExtractorConfig>::Inner, B>
where
    T: AuthExtractorConfig,
    B: CompleteErrorResponse,
{
    fn from(config: TypedConfig<T, B>) -> Self {
        Self::new(config.a.into_inner())
    }
}



impl<C, B> AuthenticationError<C, B>
where
    C: Challenge,
    B: CompleteErrorResponse,
{
    /// Construct a AuthenticationError from a config instance with ErrorResponse hint
    pub fn hinted_from<E, F, T: AuthExtractorConfig<Inner = C>, A: AuthExtractor<Error = E, Future = F, CompleteResponse = B>>(config: T, _: &A) -> Self {
        Self::new(config.into_inner())
    }
}
