# actix-limitation

> Rate limiter using a fixed window counter for arbitrary keys, backed by Redis for Actix Web.  
> Originally based on <https://github.com/fnichol/limitation>.

[![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation)
[![Documentation](https://docs.rs/actix-limitation/badge.svg?version=0.3.0)](https://docs.rs/actix-limitation/0.3.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-limitation)
[![Dependency Status](https://deps.rs/crate/actix-limitation/0.3.0/status.svg)](https://deps.rs/crate/actix-limitation/0.3.0)

## Examples

```toml
[dependencies]
actix-web = "4"
actix-limitation = "0.3"
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
