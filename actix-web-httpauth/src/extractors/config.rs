use super::AuthenticationError;
use crate::headers::www_authenticate::Challenge;

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

impl<T> From<T> for AuthenticationError<<T as AuthExtractorConfig>::Inner>
where
    T: AuthExtractorConfig,
{
    fn from(config: T) -> Self {
        AuthenticationError::new(config.into_inner())
    }
}
