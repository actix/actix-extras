use std::fmt::{Debug, Display};

use actix_web::{http::header::TryIntoHeaderValue, web::Bytes};

pub mod basic;
pub mod bearer;

/// Authentication challenge for `WWW-Authenticate` header.
pub trait Challenge: TryIntoHeaderValue + Debug + Display + Clone + Send + Sync {
    /// Converts the challenge into a bytes suitable for HTTP transmission.
    fn to_bytes(&self) -> Bytes;
}
