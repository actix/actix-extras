use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{ready, Ready};
use uuid::Uuid;

/// A unique identifier generated for each incoming request.
///
/// Extracting a `RequestId` when the `TracingLogger` middleware is not registered will result in
/// an internal server error.
///
/// # Usage
/// ```rust
/// use actix_web::get;
/// use tracing_actix_web::RequestId;
/// use uuid::Uuid;
///
/// #[get("/")]
/// async fn index(request_id: RequestId) -> String {
///   format!("{}", request_id)
/// }
///
/// #[get("/2")]
/// async fn index2(request_id: RequestId) -> String {
///  let uuid: Uuid = request_id.into();
///   format!("{}", uuid)
/// }
/// ```
#[derive(Clone, Copy)]
pub struct RequestId(Uuid);

impl RequestId {
    pub(crate) fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::ops::Deref for RequestId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::convert::Into<Uuid> for RequestId {
    fn into(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl FromRequest for RequestId {
    type Error = ();
    type Future = Ready<Result<Self, Self::Error>>;
    type Config = ();

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(req.extensions().get::<RequestId>().copied().ok_or(()))
    }
}
