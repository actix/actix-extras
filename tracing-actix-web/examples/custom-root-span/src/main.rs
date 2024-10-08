use std::{io, sync::LazyLock};

use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    web, App, Error, HttpServer,
};
use opentelemetry::{global, trace::TracerProvider, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator, runtime::TokioCurrentThread, trace::Config, Resource,
};
use opentelemetry_semantic_conventions::resource;
use tracing::Span;
use tracing_actix_web::{DefaultRootSpanBuilder, RootSpan, RootSpanBuilder, TracingLogger};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

/// We will define a custom root span builder to capture additional fields, specific
/// to our application, on top of the ones provided by `DefaultRootSpanBuilder` out of the box.
pub struct CustomRootSpanBuilder;

impl RootSpanBuilder for CustomRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span {
        // Not sure why you'd be keen to capture this, but it's an example and we try to keep it simple
        let n_headers = request.headers().len();
        // We set `cloud_provider` to a constant value.
        //
        // `name` is not known at this point - we delegate the responsibility to populate it
        // to the `personal_hello` handler. We MUST declare the field though, otherwise
        // `span.record("caller_name", XXX)` will just be silently ignored by `tracing`.
        tracing_actix_web::root_span!(
            request,
            n_headers,
            cloud_provider = "localhost",
            caller_name = tracing::field::Empty
        )
    }

    fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        // Capture the standard fields when the request finishes.
        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}

async fn hello() -> &'static str {
    "Hello world!"
}

async fn personal_hello(root_span: RootSpan, name: web::Path<String>) -> String {
    // Add more context to the root span of the request.
    root_span.record("caller_name", &name.as_str());
    format!("Hello {}!", name)
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    init_telemetry();

    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::<CustomRootSpanBuilder>::new())
            .service(web::resource("/hello").to(hello))
            .service(web::resource("/hello/{name}").to(personal_hello))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    // Ensure all spans have been shipped to Jaeger.
    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

const APP_NAME: &str = "tracing-actix-web-demo";

static RESOURCE: LazyLock<Resource> =
    LazyLock::new(|| Resource::new(vec![KeyValue::new(resource::SERVICE_NAME, APP_NAME)]));

/// Init a `tracing` subscriber that prints spans to stdout as well as
/// ships them to Jaeger.
///
/// Check the `opentelemetry` example for more details.
fn init_telemetry() {
    // Start a new otlp trace pipeline.
    // Spans are exported in batch - recommended setup for a production application.
    global::set_text_map_propagator(TraceContextPropagator::new());
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317"),
        )
        .with_trace_config(Config::default().with_resource(RESOURCE.clone()))
        .install_batch(TokioCurrentThread)
        .expect("Failed to install OpenTelemetry tracer.")
        .tracer_builder(APP_NAME)
        .build();

    // Filter based on level - trace, debug, info, warn, error
    // Tunable via `RUST_LOG` env variable
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info"));
    // Create a `tracing` layer using the otlp tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    // Create a `tracing` layer to emit spans as structured logs to stdout
    let formatting_layer = BunyanFormattingLayer::new(APP_NAME.into(), std::io::stdout);
    // Combined them all together in a `tracing` subscriber
    let subscriber = Registry::default()
        .with(env_filter)
        .with(telemetry)
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to install `tracing` subscriber.")
}
