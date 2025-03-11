# actix-cors

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-cors?label=latest)](https://crates.io/crates/actix-cors)
[![Documentation](https://docs.rs/actix-cors/badge.svg?version=0.7.1)](https://docs.rs/actix-cors/0.7.1)
![Version](https://img.shields.io/badge/rustc-1.75+-ab6000.svg)
![MIT or Apache 2.0 licensed](https://img.shields.io/crates/l/actix-cors.svg)
<br />
[![Dependency Status](https://deps.rs/crate/actix-cors/0.7.1/status.svg)](https://deps.rs/crate/actix-cors/0.7.1)
[![Download](https://img.shields.io/crates/d/actix-cors.svg)](https://crates.io/crates/actix-cors)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/NWpN5mmg3x)

<!-- prettier-ignore-end -->

<!-- cargo-rdme start -->

Cross-Origin Resource Sharing (CORS) controls for Actix Web.

This middleware can be applied to both applications and resources. Once built, a [`Cors`] builder can be used as an argument for Actix Web's `App::wrap()`, `Scope::wrap()`, or `Resource::wrap()` methods.

This CORS middleware automatically handles `OPTIONS` preflight requests.

## Crate Features

- `draft-private-network-access`: ⚠️ Unstable. Adds opt-in support for the [Private Network Access] spec extensions. This feature is unstable since it will follow breaking changes in the draft spec until it is finalized.

## Example

```rust
use actix_cors::Cors;
use actix_web::{get, http, web, App, HttpRequest, HttpResponse, HttpServer};

#[get("/index.html")]
async fn index(req: HttpRequest) -> &'static str {
    "<p>Hello World!</p>"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let cors = Cors::default()
            .allowed_origin("https://www.rust-lang.org")
            .allowed_origin_fn(|origin, _req_head| {
                origin.as_bytes().ends_with(b".rust-lang.org")
            })
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .wrap(cors)
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await;

    Ok(())
}
```

[Private Network Access]: https://wicg.github.io/private-network-access

<!-- cargo-rdme end -->

## Documentation & Resources

- [API Documentation](https://docs.rs/actix-cors)
- [Example Project](https://github.com/actix/examples/tree/master/cors)
- Minimum Supported Rust Version (MSRV): 1.75
