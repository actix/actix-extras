use actix_web::{dev::Payload, HttpMessage};
use actix_web::{FromRequest, HttpRequest, ResponseError};
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
///
/// Optionally, using the `uuid_v7` feature flag will allow [`RequestId`] to use UUID v7 instead of the currently used UUID v4.
///
/// However, the [`uuid`] crate requires a compile time flag `uuid_unstable` to be passed in `RUSTFLAGS="--cfg uuid_unstable"` in order to compile. You can read more about it [here](https://docs.rs/uuid/latest/uuid/#unstable-features).
///
#[derive(Clone, Copy, Debug)]
pub struct RequestId(Uuid);

impl RequestId {
    pub(crate) fn generate() -> Self {
        // Compiler error for providing context on requirements to enable the `uuid_v7` feature flag
        #[cfg(all(feature = "uuid_v7", not(uuid_unstable)))]
        compile_error!("feature \"uuid_v7\" requires \"uuid_unstable\" to be passed as configuration in rustflags");

        #[cfg(not(feature = "uuid_v7"))]
        {
            Self(Uuid::new_v4())
        }
        #[cfg(all(uuid_unstable, feature = "uuid_v7"))]
        {
            Self(Uuid::now_v7())
        }
    }
}

impl std::ops::Deref for RequestId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<RequestId> for Uuid {
    fn from(r: RequestId) -> Self {
        r.0
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromRequest for RequestId {
    type Error = RequestIdExtractionError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(
            req.extensions()
                .get::<RequestId>()
                .copied()
                .ok_or(RequestIdExtractionError { _priv: () }),
        )
    }
}

#[derive(Debug)]
/// Error returned by the [`RequestId`] extractor when it fails to retrieve
/// the current request id from request-local storage.
///
/// It only happens if you try to extract the current request id without having
/// registered [`TracingLogger`] as a middleware for your application.
///
/// [`TracingLogger`]: crate::TracingLogger
pub struct RequestIdExtractionError {
    // It turns out that a unit struct has a public constructor!
    // Therefore adding fields to it (either public or private) later on
    // is an API breaking change.
    // Therefore we are adding a dummy private field that the compiler is going
    // to optimise away to make sure users cannot construct this error
    // manually in their own code.
    _priv: (),
}

impl ResponseError for RequestIdExtractionError {}

impl std::fmt::Display for RequestIdExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to retrieve request id from request-local storage."
        )
    }
}

impl std::error::Error for RequestIdExtractionError {}
