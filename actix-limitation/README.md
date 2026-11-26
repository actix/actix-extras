# actix-limitation

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation)
[![Documentation](https://docs.rs/actix-limitation/badge.svg?version=0.7.0)](https://docs.rs/actix-limitation/0.7.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-limitation)
[![Dependency Status](https://deps.rs/crate/actix-limitation/0.7.0/status.svg)](https://deps.rs/crate/actix-limitation/0.7.0)

<!-- prettier-ignore-end -->

<!-- cargo-rdme start -->

Fixed-window rate limiting for arbitrary keys in Actix Web, backed by Redis or an in-memory store.

Originally based on <https://github.com/fnichol/limitation>.

## Backends

[`Limiter::builder`](https://docs.rs/actix-limitation/latest/actix_limitation/struct.Limiter.html#method.builder) uses Redis to share counters across application instances that use the same database. Enable `redis-native-tls` or `redis-rustls` for TLS.

The optional `memory-store` feature adds `Limiter::memory_builder` for process-local counters that reset on restart. Build the store outside `HttpServer::new` and clone it into each worker to share counters.

## Example

Allow five requests per peer IP in a ten-second window:

```rust
use std::time::Duration;

use actix_limitation::{Limiter, RateLimiter};
use actix_web::{web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let limiter = web::Data::new(
        Limiter::builder("redis://127.0.0.1")
            .key_by(|req| req.connection_info().peer_addr().map(str::to_owned))
            .limit(5)
            .period(Duration::from_secs(10))
            .build()
            .unwrap(),
    );

    HttpServer::new(move || {
        App::new()
            .app_data(limiter.clone())
            .wrap(RateLimiter::default())
            .default_service(web::to(|| async { "Hello!" }))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

## Running the Bundled Examples

Both bundled examples (`examples/memory.rs` and `examples/redis.rs`) allow five requests per peer IP in a ten-second window.

Run the in-memory example:

```console
cargo run --example memory --features memory-store
```

Or start Redis and run the Redis example:

```console
docker run --rm -d -p 127.0.0.1:6379:6379 redis:8
cargo run --example redis
```

From another terminal, repeat `curl -i http://127.0.0.1:8080/` six times within ten seconds. The sixth request returns HTTP 429.

<!-- cargo-rdme end -->

## Sponsors

<a href="https://www.blacksmith.sh/">
  <img src="https://raw.githubusercontent.com/actix/actix-extras/main/.github/sponsors/blacksmith.png" alt="CI powered by Blacksmith" width="280" />
</a>
