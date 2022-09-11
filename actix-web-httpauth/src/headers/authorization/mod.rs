//! `Authorization` header and various auth schemes.

mod errors;
mod header;
mod scheme;

pub use self::{
    errors::ParseError,
    header::Authorization,
    scheme::{basic::Basic, bearer::Bearer, Scheme},
};
