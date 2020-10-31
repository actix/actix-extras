//! `tracing-actix-web` provides [`TracingLogger`], a middleware to log request and response info
//! when using the [`actix-web`] framework.
//!
//! [`TracingLogger`] is designed as a drop-in replacement of [`actix-web`]'s [`Logger`].
//!
//! [`Logger`] is built on top of the [`log`] crate: you need to use regular expressions to parse
//! the request information out of the logged message.
//!
//! [`TracingLogger`] relies on [`tracing`], a modern instrumentation framework for structured
//! logging: all request information is captured as a machine-parsable set of key-value pairs.
//! It also enables propagation of context information to children spans.
//!
//! [`TracingLogger`]: struct.TracingLogger.html
//! [`actix-web`]: https://docs.rs/actix-web
//! [`Logger`]: https://docs.rs/actix-web/3.0.2/actix_web/middleware/struct.Logger.html
//! [`log`]: https://docs.rs/log
//! [`tracing`]: https://docs.rs/tracing
use actix_web::dev::{Payload, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest};
use futures::future::{ok, ready, Ready};
use futures::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tracing::Span;
use tracing_futures::Instrument;
use uuid::Uuid;

/// `TracingLogger` is a middleware to log request and response info in a structured format.
///
/// `TracingLogger` is designed as a drop-in replacement of [`actix-web`]'s [`Logger`].
///
/// [`Logger`] is built on top of the [`log`] crate: you need to use regular expressions to parse
/// the request information out of the logged message.
///
/// `TracingLogger` relies on [`tracing`], a modern instrumentation framework for structured
/// logging: all request information is captured as a machine-parsable set of key-value pairs.
/// It also enables propagation of context information to children spans.
///
/// ## Usage
///
/// Register `TracingLogger` as a middleware for your application using `.wrap` on `App`.  
/// Add a `Subscriber` implementation to output logs to the console.
///
/// ```rust
/// use actix_web::middleware::Logger;
/// use actix_web::App;
/// use tracing::{Subscriber, subscriber::set_global_default};
/// use tracing_actix_web::TracingLogger;
/// use tracing_log::LogTracer;
/// use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
/// use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
///
/// /// Compose multiple layers into a `tracing`'s subscriber.
/// pub fn get_subscriber(
///     name: String,
///     env_filter: String
/// ) -> impl Subscriber + Send + Sync {
///     let env_filter = EnvFilter::try_from_default_env()
///         .unwrap_or(EnvFilter::new(env_filter));
///     let formatting_layer = BunyanFormattingLayer::new(
///         name.into(),
///         std::io::stdout
///     );
///     Registry::default()
///         .with(env_filter)
///         .with(JsonStorageLayer)
///         .with(formatting_layer)
/// }
///
/// /// Register a subscriber as global default to process span data.
/// ///
/// /// It should only be called once!
/// pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
///     LogTracer::init().expect("Failed to set logger");
///     set_global_default(subscriber).expect("Failed to set subscriber");
/// }
///
/// fn main() {
///     let subscriber = get_subscriber("app".into(), "info".into());
///     init_subscriber(subscriber);
///
///     let app = App::new().wrap(TracingLogger);
/// }
/// ```
///
/// [`actix-web`]: https://docs.rs/actix-web
/// [`Logger`]: https://docs.rs/actix-web/3.0.2/actix_web/middleware/struct.Logger.html
/// [`log`]: https://docs.rs/log
/// [`tracing`]: https://docs.rs/tracing
pub struct TracingLogger;

impl<S, B> Transform<S> for TracingLogger
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TracingLoggerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(TracingLoggerMiddleware { service })
    }
}

#[doc(hidden)]
pub struct TracingLoggerMiddleware<S> {
    service: S,
}

/// A unique identifier for each incomming request. This ID is added to the logger span, even if
/// the `RequestId` is never extracted.
///
/// Extracting a `RequestId` when the `TracingLogger` middleware is not registered, will result in
/// a internal server error.
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

impl<S, B> Service for TracingLoggerMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let user_agent = req
            .headers()
            .get("User-Agent")
            .map(|h| h.to_str().unwrap_or(""))
            .unwrap_or("");
        let request_id = RequestId(Uuid::new_v4());
        let span = tracing::info_span!(
            "Request",
            request_path = %req.path(),
            user_agent = %user_agent,
            client_ip_address = %req.connection_info().realip_remote_addr().unwrap_or(""),
            request_id = %request_id.0,
            status_code = tracing::field::Empty,
        );
        req.extensions_mut().insert(request_id);
        let fut = self.service.call(req);
        Box::pin(
            async move {
                let outcome = fut.await;
                let status_code = match &outcome {
                    Ok(response) => response.response().status(),
                    Err(error) => error.as_response_error().status_code(),
                };
                Span::current().record("status_code", &status_code.as_u16());
                outcome
            }
            .instrument(span),
        )
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
