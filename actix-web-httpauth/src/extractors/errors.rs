use std::error::Error;
use std::fmt;

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};

use crate::headers::www_authenticate::Challenge;
use crate::headers::www_authenticate::WwwAuthenticate;

/// Authentication error returned by authentication extractors.
///
/// Different extractors may extend `AuthenticationError` implementation
/// in order to provide access to inner challenge fields.
#[derive(Debug)]
pub struct AuthenticationError<C: Challenge> {
    challenge: C,
    status_code: StatusCode,
}

impl<C: Challenge> AuthenticationError<C> {
    /// Creates new authentication error from the provided `challenge`.
    ///
    /// By default returned error will resolve into the `HTTP 401` status code.
    pub fn new(challenge: C) -> AuthenticationError<C> {
        AuthenticationError {
            challenge,
            status_code: StatusCode::UNAUTHORIZED,
        }
    }

    /// Returns mutable reference to the inner challenge instance.
    pub fn challenge_mut(&mut self) -> &mut C {
        &mut self.challenge
    }

    /// Returns mutable reference to the inner status code.
    ///
    /// Can be used to override returned status code, but by default
    /// this lib tries to stick to the RFC, so it might be unreasonable.
    pub fn status_code_mut(&mut self) -> &mut StatusCode {
        &mut self.status_code
    }
}

impl<C: Challenge> fmt::Display for AuthenticationError<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.status_code, f)
    }
}

impl<C: 'static + Challenge> Error for AuthenticationError<C> {}

impl<C: 'static + Challenge> ResponseError for AuthenticationError<C> {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code)
            // TODO: Get rid of the `.clone()`
            .insert_header(WwwAuthenticate(self.challenge.clone()))
            .finish()
    }

    fn status_code(&self) -> StatusCode {
        self.status_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headers::www_authenticate::basic::Basic;
    use actix_web::Error;

    #[test]
    fn test_status_code_is_preserved_across_error_conversions() {
        let ae: AuthenticationError<Basic> = AuthenticationError::new(Basic::default());
        let expected = ae.status_code;

        // Converting the AuthenticationError into a ResponseError should preserve the status code.
        let e = Error::from(ae);
        let re = e.as_response_error();
        assert_eq!(expected, re.status_code());
    }
}
