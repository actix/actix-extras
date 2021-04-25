use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
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

impl std::convert::Into<Span> for RootSpan {
    fn into(self) -> Span {
        self.0
    }
}

impl FromRequest for RootSpan {
    type Error = ();
    type Future = Ready<Result<Self, Self::Error>>;
    type Config = ();

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(req.extensions().get::<RootSpan>().cloned().ok_or(()))
    }
}
