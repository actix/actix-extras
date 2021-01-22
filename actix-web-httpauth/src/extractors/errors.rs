use std::borrow::Borrow;
use std::error::Error;
use std::fmt;

use actix_web::http::StatusCode;
use actix_web::{
    dev::HttpResponseBuilder, dev::ServiceRequest, FromRequest, HttpRequest,
    HttpResponse, ResponseError,
};

use crate::headers::www_authenticate::WwwAuthenticate;

use super::{AuthExtractor, AuthExtractorConfig};

/// Complete the error response, used to override the response of AuthenticationError
///
/// # Example
///
/// ```
/// struct MyErrorResponse {}
/// impl CompleteResponse for MyErrorResponse {
///   fn complete_response(builder: &mut HttpResponseBuilder) -> HttpResponse {
///     builder.message_body("Unauthorized")
///   }
/// }
///
///
/// type MyBasicAuth = BasicAuth<ApiErrorResponse>;
///
/// #[get("/api/")]
/// fn api_index(credential: MyBasicAuth) -> impl Responder {
///   debug!("Hello, {}", credential.user_id());
///   Response::Ok().json(ApiStatus::Ok)
/// }
/// ```
pub trait CompleteErrorResponse: 'static + fmt::Debug + Clone + Default {
    /// Modify the response builder and complete the response
    /// e.g. `builder.finish()`
    fn complete_response(builder: &mut HttpResponseBuilder) -> HttpResponse;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultErrorResponse {}
impl CompleteErrorResponse for DefaultErrorResponse {
    fn complete_response(builder: &mut HttpResponseBuilder) -> HttpResponse {
        builder.finish()
    }
}

/// Authentication error returned by authentication extractors.
///
/// Different extractors may extend `AuthenticationError` implementation
/// in order to provide access to inner challenge fields.
#[derive(Debug)]
pub struct AuthenticationError<T: AuthExtractorConfig> {
    challenge: <T as AuthExtractorConfig>::Inner,
    status_code: StatusCode,
}

impl<T: AuthExtractorConfig> AuthenticationError<T> {
    /// Creates new authentication error from the provided `challenge`.
    ///
    /// By default returned error will resolve into the `HTTP 401` status code.
    pub fn new2(challenge: <T as AuthExtractorConfig>::Inner) -> AuthenticationError<T> {
        AuthenticationError {
            challenge,
            status_code: StatusCode::UNAUTHORIZED,
        }
    }

    /// Creates new authentication error from the provided `challenge`.
    ///
    /// By default returned error will resolve into the `HTTP 401` status code.
    pub fn new(config: T) -> AuthenticationError<T> {
        Self::new2(config.into_inner())
    }

    /// Returns mutable reference to the inner challenge instance.
    pub fn challenge_mut(&mut self) -> &mut <T as AuthExtractorConfig>::Inner {
        &mut self.challenge
    }

    /// Returns mutable reference to the inner status code.
    ///
    /// Can be used to override returned status code, but by default
    /// this lib tries to stick to the RFC, so it might be unreasonable.
    pub fn status_code_mut(&mut self) -> &mut StatusCode {
        &mut self.status_code
    }
}

impl<T: AuthExtractorConfig> AuthenticationError<T> {
    /// Create new authentication error based on the configuration in req
    pub fn default<R: Borrow<HttpRequest>>(req: R) -> Self {
        // TODO: debug! the original error
        let challenge = req
            .borrow()
            .app_data::<T>()
            .map(|config| config.clone())
            // TODO: Add trace! about `Default::default` call
            .unwrap_or_else(Default::default);

        Self::new(challenge)
    }

    /// Create new authentication error based on the configuration in req
    pub fn default2<R: Borrow<ServiceRequest>>(req: R) -> Self {
        // TODO: debug! the original error
        let challenge = req
            .borrow()
            .app_data::<T>()
            .map(|config| config.clone())
            // TODO: Add trace! about `Default::default` call
            .unwrap_or_else(Default::default);

        Self::new(challenge)
    }

    /// Create new authentication error based on the configuration in req
    pub fn default_hinted<
        R: Borrow<HttpRequest>,
        F,
        A: AuthExtractor + FromRequest<Error = Self, Future = F, Config = T>,
    >(
        req: R,
        _: &A,
    ) -> Self {
        Self::default(req)
    }

    /// Create new authentication error based on the configuration in req
    pub fn default_hinted2<
        R: Borrow<ServiceRequest>,
        F,
        A: AuthExtractor + FromRequest<Error = Self, Future = F, Config = T>,
    >(
        req: R,
        _: &A,
    ) -> Self {
        Self::default2(req)
    }
}

impl<T: AuthExtractorConfig> fmt::Display for AuthenticationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.status_code, f)
    }
}

impl<T: AuthExtractorConfig> Error for AuthenticationError<T> {}

impl<T: AuthExtractorConfig> ResponseError for AuthenticationError<T> {
    fn error_response(&self) -> HttpResponse {
        <T as AuthExtractorConfig>::Builder::complete_response(
            HttpResponse::build(self.status_code)
                // TODO: Get rid of the `.clone()`
                .set(WwwAuthenticate(self.challenge.clone())),
        )
    }

    fn status_code(&self) -> StatusCode {
        self.status_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headers::www_authenticate::basic::Basic;
    use actix_web::Error;

    #[test]
    fn test_status_code_is_preserved_across_error_conversions() {
        let ae: AuthenticationError<Basic> = AuthenticationError::new(Basic::default());
        let expected = ae.status_code;

        // Converting the AuthenticationError into a ResponseError should preserve the status code.
        let e = Error::from(ae);
        let re = e.as_response_error();
        assert_eq!(expected, re.status_code());
    }
}
