use std::fmt;

use actix_web::http::StatusCode;

/// Bearer authorization error types, described in [RFC 6750](https://tools.ietf.org/html/rfc6750#section-3.1)
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Error {
    /// The request is missing a required parameter, includes an unsupported
    /// parameter or parameter value, repeats the same parameter, uses more
    /// than one method for including an access token, or is otherwise
    /// malformed.
    InvalidRequest,

    /// The access token provided is expired, revoked, malformed, or invalid
    /// for other reasons.
    InvalidToken,

    /// The request requires higher privileges than provided by the access
    /// token.
    InsufficientScope,
}

impl Error {
    /// Returns [HTTP status code] suitable for current error type.
    ///
    /// [HTTP status code]: `actix_web::http::StatusCode`
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn status_code(&self) -> StatusCode {
        match self {
            Error::InvalidRequest => StatusCode::BAD_REQUEST,
            Error::InvalidToken => StatusCode::UNAUTHORIZED,
            Error::InsufficientScope => StatusCode::FORBIDDEN,
        }
    }

    #[doc(hidden)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_str(&self) -> &str {
        match self {
            Error::InvalidRequest => "invalid_request",
            Error::InvalidToken => "invalid_token",
            Error::InsufficientScope => "insufficient_scope",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
