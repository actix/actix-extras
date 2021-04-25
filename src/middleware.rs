use crate::{DefaultRootSpanBuilder, RequestId, RootSpan, RootSpanBuilder};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage};
use futures::future::{ok, Ready};
use futures::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tracing::Span;
use tracing_futures::Instrument;

/// `TracingLogger` is a middleware to capture structured diagnostic when processing an HTTP request.
/// Check the crate-level documentation for an in-depth introduction.
///
/// `TracingLogger` is designed as a drop-in replacement of [`actix-web`]'s [`Logger`].
///
/// # Usage
///
/// Register `TracingLogger` as a middleware for your application using `.wrap` on `App`.  
/// In this example we add a [`tracing::Subscriber`] to output structured logs to the console.
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
///     let app = App::new().wrap(TracingLogger::default());
/// }
/// ```
///
/// [`actix-web`]: https://docs.rs/actix-web
/// [`Logger`]: https://docs.rs/actix-web/3.0.2/actix_web/middleware/struct.Logger.html
/// [`tracing`]: https://docs.rs/tracing
pub struct TracingLogger<RootSpan: RootSpanBuilder> {
    root_span_builder: std::marker::PhantomData<RootSpan>,
}

impl<RootSpan: RootSpanBuilder> Clone for TracingLogger<RootSpan> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl Default for TracingLogger<DefaultRootSpanBuilder> {
    fn default() -> Self {
        TracingLogger::new()
    }
}

impl<RootSpan: RootSpanBuilder> TracingLogger<RootSpan> {
    pub fn new() -> TracingLogger<RootSpan> {
        TracingLogger {
            root_span_builder: Default::default(),
        }
    }
}

impl<S, B, RootSpan> Transform<S, ServiceRequest> for TracingLogger<RootSpan>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
    RootSpan: RootSpanBuilder,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TracingLoggerMiddleware<S, RootSpan>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(TracingLoggerMiddleware {
            service,
            root_span_builder: std::marker::PhantomData::default(),
        })
    }
}

#[doc(hidden)]
pub struct TracingLoggerMiddleware<S, RootSpanBuilder> {
    service: S,
    root_span_builder: std::marker::PhantomData<RootSpanBuilder>,
}

impl<S, B, RootSpanType> Service<ServiceRequest> for TracingLoggerMiddleware<S, RootSpanType>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
    RootSpanType: RootSpanBuilder,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        req.extensions_mut().insert(RequestId::generate());
        let root_span = RootSpanType::on_request_start(&req);

        let root_span_wrapper = RootSpan::new(root_span.clone());
        req.extensions_mut().insert(root_span_wrapper);

        let fut = self.service.call(req);
        Box::pin(
            async move {
                let outcome = fut.await;
                RootSpanType::on_request_end(Span::current(), &outcome);

                #[cfg(feature = "emit_event_on_error")]
                if let Err(error) = &outcome {
                    let response_error = error.as_response_error();
                    let status_code = response_error.status_code();
                    let error_msg_prefix =
                        "Error encountered while processing the incoming HTTP request";
                    if status_code.is_client_error() {
                        tracing::warn!("{}: {:?}", error_msg_prefix, response_error);
                    } else {
                        tracing::error!("{}: {:?}", error_msg_prefix, response_error);
                    }
                }
                outcome
            }
            .instrument(root_span),
        )
    }
}
