use actix_web::dev::ServiceRequest;
use opentelemetry_0_14_pkg::propagation::Extractor;

pub(crate) struct RequestHeaderCarrier<'a> {
    headers: &'a actix_web::http::HeaderMap,
}

impl<'a> RequestHeaderCarrier<'a> {
    pub(crate) fn new(headers: &'a actix_web::http::HeaderMap) -> Self {
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
    use opentelemetry_0_14_pkg::trace::TraceContextExt as _;
    use tracing_opentelemetry_0_13_pkg::OpenTelemetrySpanExt as _;

    let parent_context = opentelemetry_0_14_pkg::global::get_text_map_propagator(|propagator| {
        propagator.extract(&crate::otel_0_14::RequestHeaderCarrier::new(req.headers()))
    });
    span.set_parent(parent_context);
    // If we have a remote parent span, this will be the parent's trace identifier.
    // If not, it will be the newly generated trace identifier with this request as root span.
    let trace_id = span.context().span().span_context().trace_id().to_hex();
    span.record("trace_id", &tracing::field::display(trace_id));
}
