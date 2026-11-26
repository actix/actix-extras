//! Challenge for the "Bearer" HTTP Authentication Scheme.

mod builder;
mod challenge;
mod errors;

pub use self::{builder::BearerBuilder, challenge::Bearer, errors::Error};

#[cfg(test)]
mod tests {
    use super::*;

    fn render(b: Bearer) -> String {
        format!("{b}")
    }

    #[test]
    fn to_bytes() {
        let bearer = Bearer::build()
            .error(Error::InvalidToken)
            .error_description("Subject 8740827c-2e0a-447b-9716-d73042e4039d not found")
            .finish();

        assert_eq!(
        render(bearer),
        "Bearer error=\"invalid_token\", error_description=\"Subject 8740827c-2e0a-447b-9716-d73042e4039d not found\"",
        );
    }

    #[test]
    fn bare_bearer_has_no_attributes() {
        assert_eq!(render(Bearer::default()), "Bearer");
    }

    #[test]
    fn all_attributes_comma_separated_in_order() {
        let b = Bearer::build()
            .realm("example")
            .scope("openid profile email")
            .error(Error::InsufficientScope)
            .error_description("need more \"with quoting\"")
            .error_uri("http://example.org")
            .finish();
        assert_eq!(
            render(b),
            "Bearer realm=\"example\", scope=\"openid profile email\", \
             error=\"insufficient_scope\", error_description=\"need more \\\"with quoting\\\"\", \
             error_uri=\"http://example.org\""
        );
    }

    #[test]
    fn header_value_matches_display() {
        use actix_web::http::header::TryIntoHeaderValue;

        let b = Bearer::build().realm("r").scope("s").finish();
        let hv = b.clone().try_into_value().unwrap();
        assert_eq!(hv.to_str().unwrap(), render(b));
    }
}
