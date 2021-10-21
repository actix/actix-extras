use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest, ResponseError};
use std::future::{ready, Ready};
use tracing::Span;

#[derive(Clone)]
/// The root span associated to the in-flight current request.
///
/// It can be used to populate additional properties using values computed or retrieved in the request
/// handler - see the crate-level documentation for more details.
///
/// Extracting a `RootSpan` when the `TracingLogger` middleware is not registered will result in
/// an internal server error.
///
/// # Usage
/// ```rust
/// use actix_web::get;
/// use tracing_actix_web::RootSpan;
/// use uuid::Uuid;
///
/// #[get("/")]
/// async fn index(root_span: RootSpan) -> String {
///     root_span.record("route", &"/");
///     # "Hello".to_string()
/// }
/// ```
pub struct RootSpan(Span);

impl RootSpan {
    pub(crate) fn new(span: Span) -> Self {
        Self(span)
    }
}

impl std::ops::Deref for RootSpan {
    type Target = Span;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<RootSpan> for Span {
    fn from(r: RootSpan) -> Self {
        r.0
    }
}

impl FromRequest for RootSpan {
    type Error = RootSpanExtractionError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(
            req.extensions()
                .get::<RootSpan>()
                .cloned()
                .ok_or(RootSpanExtractionError { _priv: () }),
        )
    }
}

#[derive(Debug)]
/// Error returned by the [`RootSpan`] extractor when it fails to retrieve
/// the root span from request-local storage.
///
/// It only happens if you try to extract the root span without having
/// registered [`TracingLogger`] as a middleware for your application.
///
/// [`TracingLogger`]: crate::TracingLogger
pub struct RootSpanExtractionError {
    // It turns out that a unit struct has a public constructor!
    // Therefore adding fields to it (either public or private) later on
    // is an API breaking change.
    // Therefore we are adding a dummy private field that the compiler is going
    // to optimise away to make sure users cannot construct this error
    // manually in their own code.
    _priv: (),
}

impl ResponseError for RootSpanExtractionError {}

impl std::fmt::Display for RootSpanExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to retrieve the root span from request-local storage."
        )
    }
}

impl std::error::Error for RootSpanExtractionError {}
