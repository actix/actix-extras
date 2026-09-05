# actix-limitation

> Rate limiter using a fixed window counter for arbitrary keys, backed by Redis or an in-memory store, for Actix Web.  
> Originally based on <https://github.com/fnichol/limitation>.

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation)
[![Documentation](https://docs.rs/actix-limitation/badge.svg?version=0.6.0)](https://docs.rs/actix-limitation/0.6.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-limitation)
[![Dependency Status](https://deps.rs/crate/actix-limitation/0.6.0/status.svg)](https://deps.rs/crate/actix-limitation/0.6.0)

<!-- prettier-ignore-end -->

## Backends

Counters are kept in a store, and a `Limiter` is bound to exactly one of them, chosen when it is constructed.

- **Redis**, via `Limiter::builder(redis_url)`. Counters are shared by every instance of your application. The `redis` crate is a required dependency, so this backend needs no feature flag and is the default choice.

  ```console
  cargo add actix-limitation
  ```

  Add the `redis-native-tls` feature flag to connect to Redis over a secure connection using `native-tls`, or `redis-rustls` to depend on `rustls` instead:

  ```console
  cargo add actix-limitation --features=redis-native-tls
  ```

- **In-memory**, via `Limiter::memory_builder(MemoryStore::new())`, behind the off-by-default `memory-store` feature flag. It needs no server at all, which makes it the zero-setup choice for local development, examples, and tests. Counters are process-local: they are not shared between instances and are lost on restart, so running N instances makes the effective limit `limit × N`. It is a real implementation, not a stub, and is safe in production on a single instance; see the [`MemoryStore` docs](https://docs.rs/actix-limitation/latest/actix_limitation/struct.MemoryStore.html) for the full caveats.

  ```console
  cargo add actix-limitation --features=memory-store
  ```

The bundled examples are the quickest way to try either backend. The in-memory one needs nothing installed:

```console
cargo run --example memory --features memory-store
curl -i localhost:8080 # 200 five times, then 429
```

The Redis one is the same app with one line changed, and needs a server:

```console
docker run --rm -p 6379:6379 redis:8
cargo run --example redis
```

## Examples

```toml
[dependencies]
actix-web = "4"
actix-limitation = "0.6"
```

```rust
use actix_limitation::{Limiter, RateLimiter};
use actix_session::SessionExt as _;
use actix_web::{dev::ServiceRequest, get, web, App, HttpServer, Responder};
use std::{sync::Arc, time::Duration};

#[get("/{id}/{name}")]
async fn index(info: web::Path<(u32, String)>) -> impl Responder {
    format!("Hello {}! id:{}", info.1, info.0)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let limiter = web::Data::new(
        Limiter::builder("redis://127.0.0.1")
            .key_by(|req: &ServiceRequest| {
                req.get_session()
                    .get(&"session-id")
                    .unwrap_or_else(|_| req.cookie(&"rate-api-id").map(|c| c.to_string()))
            })
            .limit(5000)
            .period(Duration::from_secs(3600)) // 60 minutes
            .build()
            .unwrap(),
    );
    HttpServer::new(move || {
        App::new()
            .wrap(RateLimiter::default())
            .app_data(limiter.clone())
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

Rate limiting by peer IP with no Redis server to run, using the `memory-store` feature:

```rust
use actix_limitation::{Limiter, MemoryStore, RateLimiter};
use actix_web::{dev::ServiceRequest, web, App, HttpServer};
use std::time::Duration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Build the limiter once, outside the `HttpServer::new` closure, then clone the `web::Data`
    // handle into each worker. The closure runs once per worker thread, so building the limiter
    // inside it would give every worker its own store and silently multiply the effective limit.
    let limiter = web::Data::new(
        Limiter::memory_builder(MemoryStore::new())
            .key_by(|req: &ServiceRequest| req.connection_info().peer_addr().map(str::to_owned))
            .limit(5)
            .period(Duration::from_secs(10))
            .build()
            .unwrap(),
    );

    HttpServer::new(move || {
        App::new()
            .wrap(RateLimiter::default())
            .app_data(limiter.clone())
            .default_service(web::to(|| async { "Hello!" }))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```
