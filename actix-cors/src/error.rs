use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use derive_more::derive::{Display, Error};

/// Errors that can occur when processing CORS guarded requests.
#[derive(Debug, Clone, Display, Error)]
#[non_exhaustive]
pub enum CorsError {
    /// Allowed origin argument must not be wildcard (`*`).
    #[display("`allowed_origin` argument must not be wildcard (`*`)")]
    WildcardOrigin,

    /// Request header `Origin` is required but was not provided.
    #[display("Request header `Origin` is required but was not provided")]
    MissingOrigin,

    /// Request header `Access-Control-Request-Method` is required but is missing.
    #[display("Request header `Access-Control-Request-Method` is required but is missing")]
    MissingRequestMethod,

    /// Request header `Access-Control-Request-Method` has an invalid value.
    #[display("Request header `Access-Control-Request-Method` has an invalid value")]
    BadRequestMethod,

    /// Request header `Access-Control-Request-Headers` has an invalid value.
    #[display("Request header `Access-Control-Request-Headers` has an invalid value")]
    BadRequestHeaders,

    /// Origin is not allowed to make this request.
    #[display("Origin is not allowed to make this request")]
    OriginNotAllowed,

    /// Request method is not allowed.
    #[display("Requested method is not allowed")]
    MethodNotAllowed,

    /// One or more request headers are not allowed.
    #[display("One or more request headers are not allowed")]
    HeadersNotAllowed,
}

impl ResponseError for CorsError {
    fn status_code(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::with_body(self.status_code(), self.to_string()).map_into_boxed_body()
    }
}
