use actix_web::{http::StatusCode, HttpResponse, ResponseError};

use derive_more::{Display, Error};

use crate::inner::add_vary_header;

/// Errors that can occur when processing CORS guarded requests.
#[derive(Debug, Clone, Display, Error)]
#[non_exhaustive]
pub enum CorsError {
    /// Allowed origin argument must not be wildcard (`*`).
    #[display(fmt = "`allowed_origin` argument must not be wildcard (`*`)")]
    WildcardOrigin,

    /// Request header `Origin` is required but was not provided.
    #[display(fmt = "Request header `Origin` is required but was not provided")]
    MissingOrigin,

    /// Request header `Access-Control-Request-Method` is required but is missing.
    #[display(fmt = "Request header `Access-Control-Request-Method` is required but is missing")]
    MissingRequestMethod,

    /// Request header `Access-Control-Request-Method` has an invalid value.
    #[display(fmt = "Request header `Access-Control-Request-Method` has an invalid value")]
    BadRequestMethod,

    /// Request header `Access-Control-Request-Headers` has an invalid value.
    #[display(fmt = "Request header `Access-Control-Request-Headers` has an invalid value")]
    BadRequestHeaders,

    /// Origin is not allowed to make this request.
    #[display(fmt = "Origin is not allowed to make this request")]
    OriginNotAllowed,

    /// Request method is not allowed.
    #[display(fmt = "Requested method is not allowed")]
    MethodNotAllowed,

    /// One or more request headers are not allowed.
    #[display(fmt = "One or more request headers are not allowed")]
    HeadersNotAllowed,
}

impl ResponseError for CorsError {
    fn status_code(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }

    fn error_response(&self) -> HttpResponse {
        let mut res = HttpResponse::with_body(self.status_code(), self.to_string());
        add_vary_header(res.headers_mut());
        res.map_into_boxed_body()
    }
}
