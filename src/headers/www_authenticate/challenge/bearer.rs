use std::str;
use std::fmt;
use std::default::Default;

use bytes::{BufMut, Bytes, BytesMut};
use actix_web::http::StatusCode;
use actix_web::http::header::{HeaderValue, IntoHeaderValue, InvalidHeaderValueBytes};

use super::Challenge;

/// Bearer authorization error types, described in [RFC 6750](https://tools.ietf.org/html/rfc6750#section-3.1)
#[derive(Debug, Copy, Clone)]
pub enum Error {
    /// The request is missing a required parameter, includes an unsupported parameter
    /// or parameter value, repeats the same parameter, uses more than one method
    /// for including an access token, or is otherwise malformed.
    InvalidRequest,

    /// The access token provided is expired, revoked, malformed, or invalid for other reasons.
    InvalidToken,

    /// The request requires higher privileges than provided by the access token.
    InsufficientScope,
}

impl Error {
    pub fn status_code(&self) -> StatusCode {
        match *self {
            Error::InvalidRequest => StatusCode::BAD_REQUEST,
            Error::InvalidToken => StatusCode::UNAUTHORIZED,
            Error::InsufficientScope => StatusCode::FORBIDDEN,
        }
    }

    fn as_str(&self) -> &'static str {
        match *self {
            Error::InvalidRequest => "invalid_request",
            Error::InvalidToken => "invalid_token",
            Error::InsufficientScope => "insufficient_scope",
        }
    }
}

/// Challenge for `WWW-Authenticate` header with HTTP Bearer auth scheme,
/// described in [RFC 6750](https://tools.ietf.org/html/rfc6750#section-3)
#[derive(Debug, Clone)]
pub struct Bearer {
    pub scope: Option<String>,
    pub realm: Option<String>,
    pub error: Option<Error>,
    pub error_description: Option<String>,
    /// It is up to implementor to provide correct absolute URI
    pub error_uri: Option<String>,
}

impl Challenge for Bearer {
    fn to_bytes(&self) -> Bytes {
        let desc_uri_required =
            self.error_description.as_ref().map_or(0, |desc| desc.len() + 20) +
            self.error_uri.as_ref().map_or(0, |url| url.len() + 12);
        let capacity = 6 +
            self.realm.as_ref().map_or(0, |realm| realm.len() + 9) +
            self.scope.as_ref().map_or(0, |scope| scope.len() + 9) +
            desc_uri_required;
        let mut buffer = BytesMut::with_capacity(capacity);
        buffer.put("Bearer");

        if let Some(ref realm) = self.realm {
            buffer.put(" realm=\"");
            buffer.put(realm);
            buffer.put("\"");
        }

        if let Some(ref scope) = self.scope {
            buffer.put(" scope=\"");
            buffer.put(scope);
            buffer.put("\"");
        }

        if let Some(ref error) = self.error {
            let error_repr = error.as_str();
            let remaining = buffer.remaining_mut();
            let required = desc_uri_required + error_repr.len() + 9; // 9 is for `" error=\"\""`
            if remaining < required {
                buffer.reserve(required);
            }
            buffer.put(" error=\"");
            buffer.put(error_repr);
            buffer.put("\"")
        }

        if let Some(ref error_description) = self.error_description {
            buffer.put(" error_description=\"");
            buffer.put(error_description);
            buffer.put("\"");
        }

        if let Some(ref error_uri) = self.error_uri {
            buffer.put(" error_uri=\"");
            buffer.put(error_uri);
            buffer.put("\"");
        }

        buffer.freeze()
    }
}

impl Default for Bearer {
    fn default() -> Self {
        Bearer {
            scope: None,
            realm: None,
            error: None,
            error_description: None,
            error_uri: None,
        }
    }
}

impl fmt::Display for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let bytes = self.to_bytes();
        let repr = str::from_utf8(&bytes)
            // Should not happen since challenges are crafted manually
            // from `&'static str`'s and Strings
            .map_err(|_| fmt::Error)?;

        f.write_str(repr)
    }
}

impl IntoHeaderValue for Bearer {
    type Error = InvalidHeaderValueBytes;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        HeaderValue::from_shared(self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_bytes() {
        let b = Bearer {
            scope: None,
            realm: None,
            error: Some(Error::InvalidToken),
            error_description: Some(String::from("Subject 8740827c-2e0a-447b-9716-d73042e4039d not found")),
            error_uri: None,
        };
        assert_eq!("Bearer error=\"invalid_token\" error_description=\"Subject 8740827c-2e0a-447b-9716-d73042e4039d not found\"",
            format!("{}", b));
    }
}
