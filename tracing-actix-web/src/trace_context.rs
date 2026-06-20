use actix_web::{dev::ServiceRequest, HttpMessage as _};

/// Trace context extracted from an incoming request.
#[derive(Clone, Debug)]
pub struct ExtractedTraceContext<Parent> {
    /// The trace identifier to record on the root span when it is known before
    /// the span is created.
    pub trace_id: Option<String>,

    /// Backend-specific parent context.
    pub parent: Parent,
}

/// Integrates a distributed tracing backend with [`TracingLogger`].
///
/// `tracing-actix-web` itself does not depend on a tracing backend. Implement
/// this trait in an adapter crate to extract remote trace context from request
/// headers and attach it to the root [`tracing::Span`].
///
/// [`TracingLogger`]: crate::TracingLogger
pub trait TraceContext: Clone + 'static {
    /// Backend-specific parent context.
    type Parent: 'static;

    /// Extract the backend-specific parent context from the incoming request.
    fn extract(&self, request: &ServiceRequest) -> ExtractedTraceContext<Self::Parent>;

    /// Attach the extracted parent context to the root span.
    fn attach(&self, parent: Self::Parent, span: &tracing::Span);
}

/// Trace context implementation used when no distributed tracing backend is
/// configured.
#[derive(Clone, Debug, Default)]
pub struct NoopTraceContext;

impl TraceContext for NoopTraceContext {
    type Parent = ();

    fn extract(&self, _request: &ServiceRequest) -> ExtractedTraceContext<Self::Parent> {
        ExtractedTraceContext {
            trace_id: None,
            parent: (),
        }
    }

    fn attach(&self, _parent: Self::Parent, _span: &tracing::Span) {}
}

#[derive(Clone, Debug)]
pub(crate) struct RequestTraceId(pub(crate) Option<String>);

pub(crate) fn insert_trace_id(request: &mut ServiceRequest, trace_id: Option<String>) {
    request.extensions_mut().insert(RequestTraceId(trace_id));
}

pub(crate) fn extract_trace_id(request: &ServiceRequest) -> Option<String> {
    request
        .extensions()
        .get::<RequestTraceId>()
        .and_then(|trace_id| trace_id.0.clone())
}
