# tracing-actix-web-opentelemetry

OpenTelemetry adapter for `tracing-actix-web`.

```toml
[dependencies]
tracing-actix-web = "0.8"
tracing-actix-web-opentelemetry = "0.32"
```

```rust
use tracing_actix_web::TracingLogger;
use tracing_actix_web_opentelemetry::OpenTelemetryTraceContext;

let middleware = TracingLogger::default()
    .with_trace_context(OpenTelemetryTraceContext::new());
```

This crate supports one OpenTelemetry crate version at a time. Use a matching
release of this adapter crate for the OpenTelemetry version used by your
application.
