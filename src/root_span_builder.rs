use crate::root_span;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use tracing::Span;

/// `RootSpanBuilder` allows you to customise the root span attached by
/// [`TracingLogger`] to incoming requests.
///
/// [`TracingLogger`]: crate::TracingLogger
pub trait RootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span;
    fn on_request_end<B>(span: Span, outcome: &Result<ServiceResponse<B>, Error>);
}

/// The default [`RootSpanBuilder`] for [`TracingLogger`].
///
/// It captures:
/// - HTTP method (`http.method`);
/// - HTTP route (`http.route`), with templated parameters;
/// - HTTP version (`http.flavor`);
/// - HTTP host (`http.host`);
/// - Client IP (`http.client_ip`);
/// - User agent (`http.user_agent`);
/// - Request path (`http.target`);
/// - Status code (`http.status_code`);
/// - [Request id](crate::RequestId) (`request_id`);
/// - [OpenTelemetry trace identifier](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/overview.md#spancontext) (`trace_id`). Empty if the feature is not enabled;
/// - OpenTelemetry span kind, set to `server` (`otel.kind`).
///
/// All field names follow [OpenTelemetry's semantic convention](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/trace/semantic_conventions).
///
/// [`TracingLogger`]: crate::TracingLogger
pub struct DefaultRootSpanBuilder;

impl RootSpanBuilder for DefaultRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span {
        root_span!(request)
    }

    fn on_request_end<B>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        match &outcome {
            Ok(response) => {
                span.record("http.status_code", &response.response().status().as_u16());
                span.record("otel.status_code", &"OK");
            }
            Err(error) => {
                let response_error = error.as_response_error();
                let status_code = response_error.status_code();
                span.record("http.status_code", &status_code.as_u16());

                if status_code.is_client_error() {
                    span.record("otel.status_code", &"OK");
                } else {
                    span.record("otel.status_code", &"ERROR");
                }
            }
        };
    }
}
