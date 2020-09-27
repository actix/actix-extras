<h1 align="center">tracing-actix-web</h1>
<div align="center">
 <strong>
   Structured logging for actix-web applications.
 </strong>
</div>

<br/>

`tracing-actix-web` provides [`TracingLogger`], a middleware to log request and response info when using the [`actix-web`] framework.

[`TracingLogger`] is designed as a drop-in replacement of [`actix-web`]'s [`Logger`].

[`Logger`] is built on top of the [`log`] crate: you need to use regular expressions to parse the request information out of the logged message.

[`TracingLogger`] relies on [`tracing`], a modern instrumentation framework for structured logging: all request information is captured as a machine-parsable set of key-value pairs.  
It also enables propagation of context information to children spans.

## How to install

Add `tracing-actix-web` to your dependencies:
```toml
[dependencies]
# ...
tracing-actix-web = "0.2"
```
If you are using [`cargo-edit`](https://github.com/killercup/cargo-edit), run
```bash
cargo add tracing-actix-web
```

`tracing-actix-web` `0.2.x` depends on `actix-web` `3.x.x`.  
If you are using `actix-web` `2.x.x` use `tracing-actix-web` `0.1.x`.

## Usage example

Register `TracingLogger` as a middleware for your application using `.wrap` on `App`.  
Add a `Subscriber` implementation to output logs to the console.

```rust
use actix_web::middleware::Logger;
use actix_web::App;
use tracing::{Subscriber, subscriber::set_global_default};
use tracing_actix_web::TracingLogger;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

/// Compose multiple layers into a `tracing`'s subscriber.
pub fn get_subscriber(
    name: String,
    env_filter: String
) -> impl Subscriber + Send + Sync {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or(EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(
        name.into(),
        std::io::stdout
    );
    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

/// Register a subscriber as global default to process span data.
///
/// It should only be called once!
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}

fn main() {
    let subscriber = get_subscriber("app".into(), "info".into());
    init_subscriber(subscriber);

    let app = App::new().wrap(TracingLogger);
}
```

[`TracingLogger`]: https://docs.rs/tracing-actix-web/0.2.0/tracing-actix-web/struct.TracingLogger.html
[`actix-web`]: https://docs.rs/actix-web
[`Logger`]: https://docs.rs/actix-web/3.0.0/actix_web/middleware/struct.Logger.html
[`log`]: https://docs.rs/log
[`tracing`]: https://docs.rs/tracing
