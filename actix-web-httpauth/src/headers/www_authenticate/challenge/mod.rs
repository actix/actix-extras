use std::fmt::{Debug, Display};

use actix_web::{http::header::TryIntoHeaderValue, web::Bytes};

pub mod basic;
pub mod bearer;

/// Authentication challenge for `WWW-Authenticate` header.
pub trait Challenge: TryIntoHeaderValue + Debug + Display + Clone + Send + Sync {
    /// Converts the challenge into a bytes suitable for HTTP transmission.
    fn to_bytes(&self) -> Bytes;
}

/// This is particularly useful for writing constructs such as
/// `AuthenticationError::new("Authentication required")`
impl Challenge for &'static str {
    fn to_bytes(&self) -> Bytes {
        (*self).into()
    }
}

#[cfg(test)]
mod tests {
    use test_strategy::proptest;

    use super::*;

    #[proptest]
    fn roundtrip_static_str(input: Box<str>) {
        // This will leak, but it's probably fine in the context of a test.  Fixable by adding:
        //    unsafe { Box::from_raw(s as *const str as *mut str); }
        let s: &'static str = Box::leak(input);
        let bytes = s.to_bytes();
        assert_eq!(s.as_bytes(), bytes);
    }
}
