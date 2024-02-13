//! `Authorization` header and various auth schemes.

mod header;
mod scheme;

pub use self::{
    header::XAPIKey,
    scheme::{api_key::APIKey, Scheme},
};
