//! Challenge for the "Bearer" HTTP Authentication Scheme.

mod builder;
mod challenge;
mod errors;

pub use self::{builder::BearerBuilder, challenge::Bearer, errors::Error};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_bytes() {
        let bearer = Bearer::build()
            .error(Error::InvalidToken)
            .error_description("Subject 8740827c-2e0a-447b-9716-d73042e4039d not found")
            .finish();

        assert_eq!(
        "Bearer error=\"invalid_token\" error_description=\"Subject 8740827c-2e0a-447b-9716-d73042e4039d not found\"",
        format!("{bearer}")
    );
    }
}
