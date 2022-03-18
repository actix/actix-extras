# Actix Limitation

Rate limiting using a fixed window counter for arbitrary keys, backed by Redis for actix-web.
This project is based on https://github.com/fnichol/limitation.

## Example
```toml
[dependencies]
actix-limitation = "0.1.3"
actix-web = "2.0.0"
actix-rt = "1.1.1"
```

Code:

```rust
use actix_web::{get, web, App, HttpServer, Responder};
use actix_limitation::{Limiter, RateLimiter};
use std::time::Duration;

#[get("/{id}/{name}/index.html")]
async fn index(info: web::Path<(u32, String)>) -> impl Responder {
    format!("Hello {}! id:{}", info.1, info.0)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let limiter = web::Data::new(
        Limiter::build("redis://127.0.0.1")
            .cookie_name("session-id")
            .session_key("rate-api-id")
            .limit(5000)
            .period(Duration::from_secs(3600)) // 60 minutes
            .finish()
            .expect("Can't build actix-limiter"),
    );

    HttpServer::new(|| {
        App::new()
            .wrap(RateLimiter)
            .app_data(limiter)
            .service(index)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```
