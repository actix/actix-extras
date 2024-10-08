use actix_web::dev::ServiceRequest;
use opentelemetry::propagation::Extractor;
#[cfg(feature = "opentelemetry_0_13")]
use opentelemetry_0_13_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_14")]
use opentelemetry_0_14_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_15")]
use opentelemetry_0_15_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_16")]
use opentelemetry_0_16_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_17")]
use opentelemetry_0_17_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_18")]
use opentelemetry_0_18_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_19")]
use opentelemetry_0_19_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_20")]
use opentelemetry_0_20_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_21")]
use opentelemetry_0_21_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_22")]
use opentelemetry_0_22_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_23")]
use opentelemetry_0_23_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_24")]
use opentelemetry_0_24_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_25")]
use opentelemetry_0_25_pkg as opentelemetry;
#[cfg(feature = "opentelemetry_0_13")]
use tracing_opentelemetry_0_12_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_14")]
use tracing_opentelemetry_0_13_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_15")]
use tracing_opentelemetry_0_14_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_16")]
use tracing_opentelemetry_0_16_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_17")]
use tracing_opentelemetry_0_17_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_18")]
use tracing_opentelemetry_0_18_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_19")]
use tracing_opentelemetry_0_19_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_20")]
use tracing_opentelemetry_0_21_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_21")]
use tracing_opentelemetry_0_22_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_22")]
use tracing_opentelemetry_0_23_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_23")]
use tracing_opentelemetry_0_24_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_24")]
use tracing_opentelemetry_0_25_pkg as tracing_opentelemetry;
#[cfg(feature = "opentelemetry_0_25")]
use tracing_opentelemetry_0_26_pkg as tracing_opentelemetry;

pub(crate) struct RequestHeaderCarrier<'a> {
    headers: &'a actix_web::http::header::HeaderMap,
}

impl<'a> RequestHeaderCarrier<'a> {
    pub(crate) fn new(headers: &'a actix_web::http::header::HeaderMap) -> Self {
        RequestHeaderCarrier { headers }
    }
}

impl<'a> Extractor for RequestHeaderCarrier<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(|header| header.as_str()).collect()
    }
}

pub(crate) fn set_otel_parent(req: &ServiceRequest, span: &tracing::Span) {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let parent_context = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&RequestHeaderCarrier::new(req.headers()))
    });
    span.set_parent(parent_context);
    // If we have a remote parent span, this will be the parent's trace identifier.
    // If not, it will be the newly generated trace identifier with this request as root span.
    #[cfg(not(any(
        feature = "opentelemetry_0_17",
        feature = "opentelemetry_0_18",
        feature = "opentelemetry_0_19",
        feature = "opentelemetry_0_20",
        feature = "opentelemetry_0_21",
        feature = "opentelemetry_0_22",
        feature = "opentelemetry_0_23",
        feature = "opentelemetry_0_24",
        feature = "opentelemetry_0_25",
    )))]
    let trace_id = span.context().span().span_context().trace_id().to_hex();

    #[cfg(any(
        feature = "opentelemetry_0_17",
        feature = "opentelemetry_0_18",
        feature = "opentelemetry_0_19",
        feature = "opentelemetry_0_20",
        feature = "opentelemetry_0_21",
        feature = "opentelemetry_0_22",
        feature = "opentelemetry_0_23",
        feature = "opentelemetry_0_24",
        feature = "opentelemetry_0_25",
    ))]
    let trace_id = {
        let id = span.context().span().span_context().trace_id();
        format!("{:032x}", id)
    };

    span.record("trace_id", &tracing::field::display(trace_id));
}
