use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use actix_web::{dev::ServiceRequest, test, web, App, HttpResponse};
use tracing::{field::Field, span, Subscriber};
use tracing_actix_web::{ExtractedTraceContext, TraceContext, TracingLogger};
use tracing_subscriber::{layer::Context, prelude::*, Layer};

#[actix_web::test]
async fn trace_context_sets_trace_id_before_span_creation_and_attaches_parent() {
    let trace_id_seen = Arc::new(AtomicBool::new(false));
    let parent_attached = Arc::new(AtomicBool::new(false));
    let subscriber = tracing_subscriber::registry().with(TraceIdLayer {
        trace_id_seen: Arc::clone(&trace_id_seen),
    });
    let _default = tracing::subscriber::set_default(subscriber);

    let app = test::init_service(
        App::new()
            .wrap(
                TracingLogger::default().with_trace_context(TestTraceContext {
                    parent_attached: Arc::clone(&parent_attached),
                }),
            )
            .route("/", web::get().to(HttpResponse::Ok)),
    )
    .await;

    let req = test::TestRequest::get().uri("/").to_request();
    let response = test::call_service(&app, req).await;

    assert!(response.status().is_success());
    assert!(trace_id_seen.load(Ordering::SeqCst));
    assert!(parent_attached.load(Ordering::SeqCst));
}

#[derive(Clone)]
struct TestTraceContext {
    parent_attached: Arc<AtomicBool>,
}

impl TraceContext for TestTraceContext {
    type Parent = ();

    fn extract(&self, _request: &ServiceRequest) -> ExtractedTraceContext<Self::Parent> {
        ExtractedTraceContext {
            trace_id: Some("trace-from-context".to_owned()),
            parent: (),
        }
    }

    fn attach(&self, _parent: Self::Parent, _span: &span::Span) {
        self.parent_attached.store(true, Ordering::SeqCst);
    }
}

struct TraceIdLayer {
    trace_id_seen: Arc<AtomicBool>,
}

impl<S> Layer<S> for TraceIdLayer
where
    S: Subscriber,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, _id: &span::Id, _ctx: Context<'_, S>) {
        let mut visitor = TraceIdVisitor::default();
        attrs.record(&mut visitor);

        if visitor.trace_id_seen {
            self.trace_id_seen.store(true, Ordering::SeqCst);
        }
    }
}

#[derive(Default)]
struct TraceIdVisitor {
    trace_id_seen: bool,
}

impl tracing::field::Visit for TraceIdVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "trace_id" && format!("{value:?}").contains("trace-from-context") {
            self.trace_id_seen = true;
        }
    }
}
