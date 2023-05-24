//! `tracing-actix-web` provides [`TracingLogger`], a middleware to collect telemetry data from applications
//! built on top of the [`actix-web`] framework.
//!
//! > `tracing-actix-web` was initially developed for the telemetry chapter of [Zero to Production In Rust](https://zero2prod.com), a hands-on introduction to backend development using the Rust programming language.
//!
//! # Getting started
//!
//! ## How to install
//!
//! Add `tracing-actix-web` to your dependencies:
//!
//! ```toml
//! [dependencies]
//! # ...
//! tracing-actix-web = "0.7"
//! tracing = "0.1"
//! actix-web = "4"
//! ```
//!
//! `tracing-actix-web` exposes three feature flags:
//!
//! - `opentelemetry_0_13`: attach [OpenTelemetry](https://github.com/open-telemetry/opentelemetry-rust)'s context to the root span using `opentelemetry` 0.13;
//! - `opentelemetry_0_14`: same as above but using `opentelemetry` 0.14;
//! - `opentelemetry_0_15`: same as above but using `opentelemetry` 0.15;
//! - `opentelemetry_0_16`: same as above but using `opentelemetry` 0.16;
//! - `opentelemetry_0_17`: same as above but using `opentelemetry` 0.17;
//! - `opentelemetry_0_18`: same as above but using `opentelemetry` 0.18;
//! - `opentelemetry_0_19`: same as above but using `opentelemetry` 0.19;
//! - `emit_event_on_error`: emit a [`tracing`] event when request processing fails with an error (enabled by default).
//!
//! ## Quickstart
//!
//! ```rust,compile_fail
//! use actix_web::{App, web, HttpServer};
//! use tracing_actix_web::TracingLogger;
//!
//! let server = HttpServer::new(|| {
//!     App::new()
//!         // Mount `TracingLogger` as a middleware
//!         .wrap(TracingLogger::default())
//!         .service( /*  */ )
//! });
//! ```
//!
//! Check out [the examples on GitHub](https://github.com/LukeMathWalker/tracing-actix-web/tree/main/examples) to get a taste of how [`TracingLogger`] can be used to observe and monitor your
//! application.  
//!
//! # From zero to hero: a crash course in observability
//!
//! ## `tracing`: who art thou?
//!
//! [`TracingLogger`] is built on top of [`tracing`], a modern instrumentation framework with
//! [a vibrant ecosystem](https://github.com/tokio-rs/tracing#related-crates).
//!
//! `tracing-actix-web`'s documentation provides a crash course in how to use [`tracing`] to instrument an `actix-web` application.  
//! If you want to learn more check out ["Are we observable yet?"](https://www.lpalmieri.com/posts/2020-09-27-zero-to-production-4-are-we-observable-yet/) -
//! it provides an in-depth introduction to the crate and the problems it solves within the bigger picture of [observability](https://docs.honeycomb.io/learning-about-observability/).
//!
//! ## The root span
//!
//! [`tracing::Span`] is the key abstraction in [`tracing`]: it represents a unit of work in your system.  
//! A [`tracing::Span`] has a beginning and an end. It can include one or more **child spans** to represent sub-unit
//! of works within a larger task.
//!
//! When your application receives a request, [`TracingLogger`] creates a new span - we call it the **[root span]**.  
//! All the spans created _while_ processing the request will be children of the root span.
//!
//! [`tracing`] empowers us to attach structured properties to a span as a collection of key-value pairs.  
//! Those properties can then be queried in a variety of tools (e.g. ElasticSearch, Honeycomb, DataDog) to
//! understand what is happening in your system.  
//!
//! ## Customisation via [`RootSpanBuilder`]
//!
//! Troubleshooting becomes much easier when the root span has a _rich context_ - e.g. you can understand most of what
//! happened when processing the request just by looking at the properties attached to the corresponding root span.  
//!
//! You might have heard of this technique as the [canonical log line pattern](https://stripe.com/blog/canonical-log-lines),
//! popularised by Stripe. It is more recently discussed in terms of [high-cardinality events](https://www.honeycomb.io/blog/observability-a-manifesto/)
//! by Honeycomb and other vendors in the observability space.
//!
//! [`TracingLogger`] gives you a chance to use the very same pattern: you can customise the properties attached
//! to the root span in order to capture the context relevant to your specific domain.
//!
//! [`TracingLogger::default`] is equivalent to:
//!
//! ```rust
//! use tracing_actix_web::{TracingLogger, DefaultRootSpanBuilder};
//!
//! // Two ways to initialise TracingLogger with the default root span builder
//! let default = TracingLogger::default();
//! let another_way = TracingLogger::<DefaultRootSpanBuilder>::new();
//! ```
//!
//! We are delegating the construction of the root span to [`DefaultRootSpanBuilder`].  
//! [`DefaultRootSpanBuilder`] captures, out of the box, several dimensions that are usually relevant when looking at an HTTP
//! API: method, version, route, etc. - check out its documentation for an extensive list.
//!
//! You can customise the root span by providing your own implementation of the [`RootSpanBuilder`] trait.  
//! Let's imagine, for example, that our system cares about a client identifier embedded inside an authorization header.
//! We could add a `client_id` property to the root span using a custom builder, `DomainRootSpanBuilder`:
//!
//! ```rust
//! use actix_web::body::MessageBody;
//! use actix_web::dev::{ServiceResponse, ServiceRequest};
//! use actix_web::Error;
//! use tracing_actix_web::{TracingLogger, DefaultRootSpanBuilder, RootSpanBuilder};
//! use tracing::Span;
//!
//! pub struct DomainRootSpanBuilder;
//!
//! impl RootSpanBuilder for DomainRootSpanBuilder {
//!     fn on_request_start(request: &ServiceRequest) -> Span {
//!         let client_id: &str = todo!("Somehow extract it from the authorization header");
//!         tracing::info_span!("Request", client_id)
//!     }
//!
//!     fn on_request_end<B: MessageBody>(_span: Span, _outcome: &Result<ServiceResponse<B>, Error>) {}
//! }
//!
//! let custom_middleware = TracingLogger::<DomainRootSpanBuilder>::new();
//! ```
//!
//! There is an issue, though: `client_id` is the _only_ property we are capturing.  
//! With `DomainRootSpanBuilder`, as it is, we do not get any of that useful HTTP-related information provided by
//! [`DefaultRootSpanBuilder`].  
//!
//! We can do better!
//!
//! ```rust
//! use actix_web::body::MessageBody;
//! use actix_web::dev::{ServiceResponse, ServiceRequest};
//! use actix_web::Error;
//! use tracing_actix_web::{TracingLogger, DefaultRootSpanBuilder, RootSpanBuilder};
//! use tracing::Span;
//!
//! pub struct DomainRootSpanBuilder;
//!
//! impl RootSpanBuilder for DomainRootSpanBuilder {
//!     fn on_request_start(request: &ServiceRequest) -> Span {
//!         let client_id: &str = todo!("Somehow extract it from the authorization header");
//!         tracing_actix_web::root_span!(request, client_id)
//!     }
//!
//!     fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
//!         DefaultRootSpanBuilder::on_request_end(span, outcome);
//!     }
//! }
//!
//! let custom_middleware = TracingLogger::<DomainRootSpanBuilder>::new();
//! ```
//!
//! [`root_span!`] is a macro provided by `tracing-actix-web`: it creates a new span by combining all the HTTP properties tracked
//! by [`DefaultRootSpanBuilder`] with the custom ones you specify when calling it (e.g. `client_id` in our example).  
//!
//! We need to use a macro because `tracing` requires all the properties attached to a span to be declared upfront, when the span is created.  
//! You cannot add new ones afterwards. This makes it extremely fast, but it pushes us to reach for macros when we need some level of
//! composition.
//!
//! [`root_span!`] exposes more or less the same knob you can find on `tracing`'s `span!` macro. You can, for example, customise
//! the span level:
//!
//! ```rust
//! use actix_web::body::MessageBody;
//! use actix_web::dev::{ServiceResponse, ServiceRequest};
//! use actix_web::Error;
//! use tracing_actix_web::{TracingLogger, DefaultRootSpanBuilder, RootSpanBuilder, Level};
//! use tracing::Span;
//!
//! pub struct CustomLevelRootSpanBuilder;
//!
//! impl RootSpanBuilder for CustomLevelRootSpanBuilder {
//!     fn on_request_start(request: &ServiceRequest) -> Span {
//!         let level = if request.path() == "/health_check" {
//!             Level::DEBUG
//!         } else {
//!             Level::INFO
//!         };
//!         tracing_actix_web::root_span!(level = level, request)
//!     }
//!
//!     fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
//!         DefaultRootSpanBuilder::on_request_end(span, outcome);
//!     }
//! }
//!
//! let custom_middleware = TracingLogger::<CustomLevelRootSpanBuilder>::new();
//! ```
//!
//! ## The [`RootSpan`] extractor
//!
//! It often happens that not all information about a task is known upfront, encoded in the incoming request.  
//! You can use the [`RootSpan`] extractor to grab the root span in your handlers and attach more information
//! to your root span as it becomes available:
//!
//! ```rust
//! use actix_web::body::MessageBody;
//! use actix_web::dev::{ServiceResponse, ServiceRequest};
//! use actix_web::{Error, HttpResponse};
//! use tracing_actix_web::{RootSpan, DefaultRootSpanBuilder, RootSpanBuilder};
//! use tracing::Span;
//! use actix_web::get;
//! use tracing_actix_web::RequestId;
//! use uuid::Uuid;
//!
//! #[get("/")]
//! async fn handler(root_span: RootSpan) -> HttpResponse {
//!     let application_id: &str = todo!("Some domain logic");
//!     // Record the property value against the root span
//!     root_span.record("application_id", &application_id);
//!
//!     // [...]
//!     # todo!()
//! }
//!
//! pub struct DomainRootSpanBuilder;
//!
//! impl RootSpanBuilder for DomainRootSpanBuilder {
//!     fn on_request_start(request: &ServiceRequest) -> Span {
//!         let client_id: &str = todo!("Somehow extract it from the authorization header");
//!         // All fields you want to capture must be declared upfront.
//!         // If you don't know the value (yet), use tracing's `Empty`
//!         tracing_actix_web::root_span!(
//!             request,
//!             client_id, application_id = tracing::field::Empty
//!         )
//!     }
//!
//!     fn on_request_end<B: MessageBody>(span: Span, response: &Result<ServiceResponse<B>, Error>) {
//!         DefaultRootSpanBuilder::on_request_end(span, response);
//!     }
//! }
//! ```
//!
//! # Unique identifiers
//!
//! ## Request Id
//!
//! `tracing-actix-web` generates a unique identifier for each incoming request, the **request id**.
//!
//! You can extract the request id using the [`RequestId`] extractor:
//!
//! ```rust
//! use actix_web::get;
//! use tracing_actix_web::RequestId;
//! use uuid::Uuid;
//!
//! #[get("/")]
//! async fn index(request_id: RequestId) -> String {
//!     format!("{}", request_id)
//! }
//! ```
//!
//! The request id is meant to identify all operations related to a particular request **within the boundary of your API**.
//! If you need to **trace** a request across multiple services (e.g. in a microservice architecture), you want to look at the `trace_id` field - see the next section on OpenTelemetry for more details.
//!
//! ## Trace Id
//!
//! To fulfill a request you often have to perform additional I/O operations - e.g. calls to other REST or gRPC APIs, database queries, etc.
//! **Distributed tracing** is the standard approach to **trace** a single request across the entirety of your stack.
//!
//! `tracing-actix-web` provides support for distributed tracing by supporting the [OpenTelemetry standard](https://opentelemetry.io/).  
//! `tracing-actix-web` follows [OpenTelemetry's semantic convention](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/overview.md#spancontext)
//! for field names.
//! Furthermore, it provides an `opentelemetry_0_17` feature flag to automatically performs trace propagation: it tries to extract the OpenTelemetry context out of the headers of incoming requests and, when it finds one, it sets it as the remote context for the current root span. The context is then propagated to your downstream dependencies if your HTTP or gRPC clients are OpenTelemetry-aware - e.g. using [`reqwest-middleware` and `reqwest-tracing`](https://github.com/TrueLayer/reqwest-middleware) if you are using `reqwest` as your HTTP client.  
//! You can then find all logs for the same request across all the services it touched by looking for the `trace_id`, automatically logged by `tracing-actix-web`.
//!
//! If you add [`tracing-opentelemetry::OpenTelemetryLayer`](https://docs.rs/tracing-opentelemetry/0.17.0/tracing_opentelemetry/struct.OpenTelemetryLayer.html)
//! in your `tracing::Subscriber` you will be able to export the root span (and all its children) as OpenTelemetry spans.
//!
//! Check out the [relevant example in the GitHub repository](https://github.com/LukeMathWalker/tracing-actix-web/tree/main/examples/opentelemetry) for reference.
//!
//! [root span]: crate::RootSpan
//! [`actix-web`]: https://docs.rs/actix-web/4.0.0-beta.13/actix_web/index.html
mod middleware;
mod request_id;
mod root_span;
mod root_span_builder;

pub use middleware::{StreamSpan, TracingLogger};
pub use request_id::RequestId;
pub use root_span::RootSpan;
pub use root_span_builder::{DefaultRootSpanBuilder, RootSpanBuilder};
// Re-exporting the `Level` enum since it's used in our `root_span!` macro
pub use tracing::Level;

#[doc(hidden)]
pub mod root_span_macro;

#[cfg(any(
    feature = "opentelemetry_0_13",
    feature = "opentelemetry_0_14",
    feature = "opentelemetry_0_15",
    feature = "opentelemetry_0_16",
    feature = "opentelemetry_0_17",
    feature = "opentelemetry_0_18",
    feature = "opentelemetry_0_19"
))]
mod otel;

#[cfg(all(feature = "opentelemetry_0_13", feature = "opentelemetry_0_14"))]
compile_error!("feature \"opentelemetry_0_13\" and feature \"opentelemetry_0_14\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_13", feature = "opentelemetry_0_15"))]
compile_error!("feature \"opentelemetry_0_13\" and feature \"opentelemetry_0_15\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_13", feature = "opentelemetry_0_16"))]
compile_error!("feature \"opentelemetry_0_13\" and feature \"opentelemetry_0_16\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_13", feature = "opentelemetry_0_17"))]
compile_error!("feature \"opentelemetry_0_13\" and feature \"opentelemetry_0_17\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_13", feature = "opentelemetry_0_18"))]
compile_error!("feature \"opentelemetry_0_13\" and feature \"opentelemetry_0_18\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_13", feature = "opentelemetry_0_19"))]
compile_error!("feature \"opentelemetry_0_13\" and feature \"opentelemetry_0_19\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_14", feature = "opentelemetry_0_15"))]
compile_error!("feature \"opentelemetry_0_14\" and feature \"opentelemetry_0_15\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_14", feature = "opentelemetry_0_16"))]
compile_error!("feature \"opentelemetry_0_14\" and feature \"opentelemetry_0_16\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_14", feature = "opentelemetry_0_17"))]
compile_error!("feature \"opentelemetry_0_14\" and feature \"opentelemetry_0_17\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_14", feature = "opentelemetry_0_18"))]
compile_error!("feature \"opentelemetry_0_14\" and feature \"opentelemetry_0_18\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_14", feature = "opentelemetry_0_19"))]
compile_error!("feature \"opentelemetry_0_14\" and feature \"opentelemetry_0_19\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_15", feature = "opentelemetry_0_16"))]
compile_error!("feature \"opentelemetry_0_15\" and feature \"opentelemetry_0_16\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_15", feature = "opentelemetry_0_17"))]
compile_error!("feature \"opentelemetry_0_15\" and feature \"opentelemetry_0_17\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_15", feature = "opentelemetry_0_18"))]
compile_error!("feature \"opentelemetry_0_15\" and feature \"opentelemetry_0_18\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_15", feature = "opentelemetry_0_19"))]
compile_error!("feature \"opentelemetry_0_15\" and feature \"opentelemetry_0_19\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_16", feature = "opentelemetry_0_17"))]
compile_error!("feature \"opentelemetry_0_16\" and feature \"opentelemetry_0_17\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_16", feature = "opentelemetry_0_18"))]
compile_error!("feature \"opentelemetry_0_16\" and feature \"opentelemetry_0_18\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_16", feature = "opentelemetry_0_19"))]
compile_error!("feature \"opentelemetry_0_16\" and feature \"opentelemetry_0_19\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_17", feature = "opentelemetry_0_18"))]
compile_error!("feature \"opentelemetry_0_17\" and feature \"opentelemetry_0_18\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_17", feature = "opentelemetry_0_19"))]
compile_error!("feature \"opentelemetry_0_17\" and feature \"opentelemetry_0_19\" cannot be enabled at the same time");

#[cfg(all(feature = "opentelemetry_0_18", feature = "opentelemetry_0_19"))]
compile_error!("feature \"opentelemetry_0_18\" and feature \"opentelemetry_0_19\" cannot be enabled at the same time");
