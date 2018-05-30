use std::fmt::{Debug, Display};

use bytes::Bytes;
use actix_web::http::header::IntoHeaderValue;

pub mod basic;
pub mod bearer;

/// Authentication challenge for `WWW-Authenticate` header.
pub trait Challenge: IntoHeaderValue + Debug + Display + Clone + Send + Sync  {
    fn to_bytes(&self) -> Bytes;
}
