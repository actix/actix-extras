use crate::{DefaultRootSpanBuilder, RequestId, RootSpan, RootSpanBuilder};
use actix_web::body::{BodySize, MessageBody};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::StatusCode;
use actix_web::web::Bytes;
use actix_web::{Error, HttpMessage, ResponseError};
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::Span;

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
/// Like [`actix-web`]'s [`Logger`], in order to use `TracingLogger` inside a Scope, Resource, or
/// Condition, the [`Compat`] middleware must be used.
///
/// ```rust
/// use actix_web::middleware::Compat;
/// use actix_web::{web, App};
/// use tracing_actix_web::TracingLogger;
///
/// let app = App::new()
///     .service(
///         web::scope("/some/route")
///             .wrap(Compat::new(TracingLogger::default())),
///     );
/// ```
///
/// [`actix-web`]: https://docs.rs/actix-web
/// [`Logger`]: https://docs.rs/actix-web/4.0.0-beta.13/actix_web/middleware/struct.Logger.html
/// [`Compat`]: https://docs.rs/actix-web/4.0.0-beta.13/actix_web/middleware/struct.Compat.html
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
    B: MessageBody + 'static,
    RootSpan: RootSpanBuilder,
{
    type Response = ServiceResponse<StreamSpan<B>>;
    type Error = Error;
    type Transform = TracingLoggerMiddleware<S, RootSpan>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TracingLoggerMiddleware {
            service,
            root_span_builder: std::marker::PhantomData,
        }))
    }
}

#[doc(hidden)]
pub struct TracingLoggerMiddleware<S, RootSpanBuilder> {
    service: S,
    root_span_builder: std::marker::PhantomData<RootSpanBuilder>,
}

#[allow(clippy::type_complexity)]
impl<S, B, RootSpanType> Service<ServiceRequest> for TracingLoggerMiddleware<S, RootSpanType>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
    RootSpanType: RootSpanBuilder,
{
    type Response = ServiceResponse<StreamSpan<B>>;
    type Error = Error;
    type Future = TracingResponse<S::Future, RootSpanType>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        req.extensions_mut().insert(RequestId::generate());
        let root_span = RootSpanType::on_request_start(&req);

        let root_span_wrapper = RootSpan::new(root_span.clone());
        req.extensions_mut().insert(root_span_wrapper);

        let fut = root_span.in_scope(|| self.service.call(req));

        TracingResponse {
            fut,
            span: root_span,
            _root_span_type: std::marker::PhantomData,
        }
    }
}

#[doc(hidden)]
#[pin_project::pin_project]
pub struct TracingResponse<F, RootSpanType> {
    #[pin]
    fut: F,
    span: Span,
    _root_span_type: std::marker::PhantomData<RootSpanType>,
}

#[doc(hidden)]
#[pin_project::pin_project]
pub struct StreamSpan<B> {
    #[pin]
    body: B,
    span: Span,
}

impl<F, B, RootSpanType> Future for TracingResponse<F, RootSpanType>
where
    F: Future<Output = Result<ServiceResponse<B>, Error>>,
    B: MessageBody + 'static,
    RootSpanType: RootSpanBuilder,
{
    type Output = Result<ServiceResponse<StreamSpan<B>>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let fut = this.fut;
        let span = this.span;

        span.in_scope(|| match fut.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(outcome) => {
                RootSpanType::on_request_end(Span::current(), &outcome);

                #[cfg(feature = "emit_event_on_error")]
                {
                    emit_event_on_error(&outcome);
                }

                Poll::Ready(outcome.map(|service_response| {
                    service_response.map_body(|_, body| StreamSpan {
                        body,
                        span: span.clone(),
                    })
                }))
            }
        })
    }
}

impl<B> MessageBody for StreamSpan<B>
where
    B: MessageBody,
{
    type Error = B::Error;

    fn size(&self) -> BodySize {
        self.body.size()
    }

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Bytes, Self::Error>>> {
        let this = self.project();

        let body = this.body;
        let span = this.span;
        span.in_scope(|| body.poll_next(cx))
    }
}

fn emit_event_on_error<B: 'static>(outcome: &Result<ServiceResponse<B>, actix_web::Error>) {
    match outcome {
        Ok(response) => {
            if let Some(err) = response.response().error() {
                // use the status code already constructed for the outgoing HTTP response
                emit_error_event(err.as_response_error(), response.status())
            }
        }
        Err(error) => {
            let response_error = error.as_response_error();
            emit_error_event(response_error, response_error.status_code())
        }
    }
}

fn emit_error_event(response_error: &dyn ResponseError, status_code: StatusCode) {
    let error_msg_prefix = "Error encountered while processing the incoming HTTP request";
    if status_code.is_client_error() {
        tracing::warn!("{}: {:?}", error_msg_prefix, response_error);
    } else {
        tracing::error!("{}: {:?}", error_msg_prefix, response_error);
    }
}
