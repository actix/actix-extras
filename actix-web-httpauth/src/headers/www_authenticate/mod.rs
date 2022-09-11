//! `WWW-Authenticate` header and various auth challenges.

mod challenge;
mod header;

pub use self::{
    challenge::{basic, bearer, Challenge},
    header::WwwAuthenticate,
};
