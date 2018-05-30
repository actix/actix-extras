use headers::www_authenticate::Challenge;

use super::AuthenticationError;

pub trait ExtractorConfig {
    type Inner: Challenge;

    fn into_inner(self) -> Self::Inner;
}

impl<T> From<T> for AuthenticationError<<T as ExtractorConfig>::Inner> where T: ExtractorConfig {
    fn from(config: T) -> Self {
        AuthenticationError::new(config.into_inner())
    }
}
