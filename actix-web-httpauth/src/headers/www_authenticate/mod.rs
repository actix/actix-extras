//! `WWW-Authenticate` header and various auth challenges

mod challenge;
mod header;

pub use self::challenge::basic;
pub use self::challenge::bearer;
pub use self::challenge::Challenge;
pub use self::header::WwwAuthenticate;
