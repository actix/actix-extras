use std::str;
use std::fmt;
use std::error::Error;

use actix_web::{HttpResponse, ResponseError};
use actix_web::http::StatusCode;

use headers::www_authenticate::{WWWAuthenticate};
use headers::www_authenticate::Challenge;

/// Authentication error returned by Auth extractor.
///
/// Different extractors may extend `AuthenticationError` implementation
/// in order to provide access to inner challenge fields.
#[derive(Debug)]
pub struct AuthenticationError<C: Challenge> {
    challenge: C,
    status_code: StatusCode,
}

impl<C: Challenge> AuthenticationError<C> {
    pub fn new(challenge: C) -> AuthenticationError<C> {
        AuthenticationError {
            challenge,
            status_code: StatusCode::UNAUTHORIZED,
        }
    }

    pub fn challenge_mut(&mut self) -> &mut C {
        &mut self.challenge
    }

    pub fn status_code_mut(&mut self) -> &mut StatusCode {
        &mut self.status_code
    }
}

impl<C: Challenge> fmt::Display for AuthenticationError<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.challenge.to_bytes();
        let repr = str::from_utf8(&bytes)
            // Should not happen since challenges are crafted manually
            // from `&'static str`'s and Strings
            .map_err(|_| fmt::Error)?;

        f.write_str(repr)
    }
}

impl<C: 'static + Challenge> Error for AuthenticationError<C> {
    fn description(&self) -> &str {
        unimplemented!()
    }
}

impl<C: 'static + Challenge> ResponseError for AuthenticationError<C> {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code)
            // TODO: Get rid of the `.clone()`
            .set(WWWAuthenticate(self.challenge.clone()))
            .finish()
    }
}
