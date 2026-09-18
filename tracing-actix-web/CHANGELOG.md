# Changelog

## Unreleased

- Remove built-in `opentelemetry_0_*` feature flags and OpenTelemetry dependencies from `tracing-actix-web`.
  - Please use `tracing-actix-web-opentelemetry` as the OpenTelemetry adapter crate instead.
- Add a backend-neutral `TraceContext` hook to attach distributed tracing context to root spans.

## 0.7.22

- Minimum supported Rust version (MSRV) is now 1.88.
- Ensure the OpenTelemetry `trace_id` is populated when request spans are created. [#724]
- Support OpenTelemetry 0.32.

[#724]: https://github.com/actix/actix-extras/pull/724

## 0.7.21

- The repository has been moved under the [actix](https://github.com/actix/actix-extras) organization. The future development will happen there.

## 0.7.20

- Support Opentelemetry 0.31
