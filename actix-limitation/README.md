# actix-limitation

> Rate limiter using a fixed window counter for arbitrary keys, backed by Redis for Actix Web.  
> Originally based on <https://github.com/fnichol/limitation>.

[![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation)
[![Documentation](https://docs.rs/actix-limitation/badge.svg?version=0.2.0)](https://docs.rs/actix-limitation/0.2.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-limitation)
[![Dependency Status](https://deps.rs/crate/actix-limitation/0.2.0/status.svg)](https://deps.rs/crate/actix-limitation/0.2.0)

## Examples

```toml
[dependencies]
actix-web = "4"
actix-limitation = "0.1.4"
```

```rust
use std::time::Duration;
use actix_web::{get, web, App, HttpServer, Responder};
use actix_limitation::{Limiter, RateLimiter};

#[get("/{id}/{name}")]
async fn index(info: web::Path<(u32, String)>) -> impl Responder {
    format!("Hello {}! id:{}", info.1, info.0)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let limiter = web::Data::new(
        Limiter::build("redis://127.0.0.1")
            .cookie_name("session-id".to_owned())
            .session_key("rate-api-id".to_owned())
            .limit(5000)
            .period(Duration::from_secs(3600)) // 60 minutes
            .finish()
            .expect("Can't build actix-limiter"),
    );

    HttpServer::new(move || {
        App::new()
            .wrap(RateLimiter)
            .app_data(limiter.clone())
            .service(index)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```
