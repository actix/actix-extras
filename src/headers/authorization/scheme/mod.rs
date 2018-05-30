use std::fmt::{Debug, Display};

use actix_web::http::header::{IntoHeaderValue, HeaderValue};

pub mod basic;
pub mod bearer;

use headers::authorization::errors::ParseError;

/// Authentication scheme for [`Authorization`](./struct.Authorization.html) header.
pub trait Scheme: IntoHeaderValue + Debug + Display + Clone + Send + Sync  {
    fn parse(header: &HeaderValue) -> Result<Self, ParseError>;
}
