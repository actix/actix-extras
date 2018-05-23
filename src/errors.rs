use std::fmt;
use std::error::Error as StdError;

use actix_web::HttpResponse;
use actix_web::error::ResponseError;
use actix_web::http::{StatusCode, header};

use basic::Config;

#[derive(Debug)]
pub struct Error {
    challenge: Config,
}

impl Error {
    pub fn new(config: Config) -> Error {
        Error {
            challenge: config,
        }
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(StatusCode::UNAUTHORIZED)
            .header(header::WWW_AUTHENTICATE, &self.challenge)
            .finish()
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        "Unauthorized request"
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}
