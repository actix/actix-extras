//! OpenTelemetry adapter for `tracing-actix-web`.
//!
//! This crate intentionally supports one OpenTelemetry crate version at a time.
//! To use a different OpenTelemetry version, select a matching release of this
//! adapter crate.

use actix_web::{dev::ServiceRequest, http::header::HeaderMap};
use opentelemetry::{global, propagation::Extractor, trace::TraceContextExt as _};
use tracing_actix_web::{ExtractedTraceContext, TraceContext};
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

/// OpenTelemetry trace context adapter for [`tracing_actix_web::TracingLogger`].
#[derive(Clone, Debug, Default)]
pub struct OpenTelemetryTraceContext;

impl OpenTelemetryTraceContext {
    /// Creates a new OpenTelemetry trace context adapter.
    pub fn new() -> Self {
        Self
    }
}

impl TraceContext for OpenTelemetryTraceContext {
    type Parent = opentelemetry::Context;

    fn extract(&self, request: &ServiceRequest) -> ExtractedTraceContext<Self::Parent> {
        let parent = global::get_text_map_propagator(|propagator| {
            propagator.extract(&RequestHeaderCarrier::new(request.headers()))
        });
        let span = parent.span();
        let span_context = span.span_context();
        let trace_id = span_context
            .is_valid()
            .then(|| format!("{:032x}", span_context.trace_id()));

        ExtractedTraceContext { trace_id, parent }
    }

    fn attach(&self, parent: Self::Parent, span: &tracing::Span) {
        let _ = span.set_parent(parent);

        let context = span.context();
        let otel_span = context.span();
        let span_context = otel_span.span_context();
        if !span_context.is_valid() {
            return;
        }

        let trace_id = span_context.trace_id();
        span.record(
            "trace_id",
            tracing::field::display(format!("{trace_id:032x}")),
        );
    }
}

struct RequestHeaderCarrier<'a> {
    headers: &'a HeaderMap,
}

impl<'a> RequestHeaderCarrier<'a> {
    fn new(headers: &'a HeaderMap) -> Self {
        Self { headers }
    }
}

impl Extractor for RequestHeaderCarrier<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(|header| header.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fmt,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use opentelemetry::trace::{
        SpanContext, SpanId, TraceContextExt as _, TraceFlags, TraceId, TraceState,
        TracerProvider as _,
    };
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing::{field::Field, span, Subscriber};
    use tracing_actix_web::TraceContext as _;
    use tracing_opentelemetry::layer;
    use tracing_subscriber::{layer::Context, prelude::*, Layer};

    use super::OpenTelemetryTraceContext;

    #[test]
    fn attach_records_generated_trace_id_when_opentelemetry_layer_is_configured() {
        let nonzero_trace_id_recorded = Arc::new(AtomicBool::new(false));
        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("test");
        let subscriber = tracing_subscriber::registry()
            .with(layer().with_tracer(tracer))
            .with(TraceIdRecordLayer {
                nonzero_trace_id_recorded: Arc::clone(&nonzero_trace_id_recorded),
                zero_trace_id_recorded: Arc::new(AtomicBool::new(false)),
            });
        let _default = tracing::subscriber::set_default(subscriber);

        let span = tracing::info_span!("request", trace_id = tracing::field::Empty);

        OpenTelemetryTraceContext::new().attach(opentelemetry::Context::new(), &span);

        assert!(nonzero_trace_id_recorded.load(Ordering::SeqCst));
    }

    #[test]
    fn attach_does_not_record_invalid_trace_id_when_opentelemetry_layer_is_missing() {
        let zero_trace_id_recorded = Arc::new(AtomicBool::new(false));
        let subscriber = tracing_subscriber::registry().with(TraceIdRecordLayer {
            nonzero_trace_id_recorded: Arc::new(AtomicBool::new(false)),
            zero_trace_id_recorded: Arc::clone(&zero_trace_id_recorded),
        });
        let _default = tracing::subscriber::set_default(subscriber);

        let parent = opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            TraceId::from(42),
            SpanId::from(7),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        ));
        let span = tracing::info_span!("request", trace_id = "remote-trace-id");

        OpenTelemetryTraceContext::new().attach(parent, &span);

        assert!(!zero_trace_id_recorded.load(Ordering::SeqCst));
    }

    struct TraceIdRecordLayer {
        nonzero_trace_id_recorded: Arc<AtomicBool>,
        zero_trace_id_recorded: Arc<AtomicBool>,
    }

    impl<S> Layer<S> for TraceIdRecordLayer
    where
        S: Subscriber,
    {
        fn on_record(&self, _id: &span::Id, values: &span::Record<'_>, _ctx: Context<'_, S>) {
            let mut visitor = TraceIdRecordVisitor::default();
            values.record(&mut visitor);

            if visitor.nonzero_trace_id_recorded {
                self.nonzero_trace_id_recorded.store(true, Ordering::SeqCst);
            }
            if visitor.zero_trace_id_recorded {
                self.zero_trace_id_recorded.store(true, Ordering::SeqCst);
            }
        }
    }

    #[derive(Default)]
    struct TraceIdRecordVisitor {
        nonzero_trace_id_recorded: bool,
        zero_trace_id_recorded: bool,
    }

    impl tracing::field::Visit for TraceIdRecordVisitor {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            if field.name() != "trace_id" {
                return;
            }

            let value = format!("{value:?}");
            if value.contains("00000000000000000000000000000000") {
                self.zero_trace_id_recorded = true;
            } else {
                self.nonzero_trace_id_recorded = true;
            }
        }
    }
}
