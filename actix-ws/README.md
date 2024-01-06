# Actix WS (Next Gen)

> WebSockets for Actix Web, without actors.

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-ws?label=latest)](https://crates.io/crates/actix-ws)
[![Documentation](https://docs.rs/actix-ws/badge.svg?version=0.2.0)](https://docs.rs/actix-ws/0.2.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-ws)
[![Dependency Status](https://deps.rs/crate/actix-ws/0.2.0/status.svg)](https://deps.rs/crate/actix-ws/0.2.0)

<!-- prettier-ignore-end -->

## Documentation & Resources

- [API Documentation](https://docs.rs/actix-ws)
- [Example Chat Project](https://github.com/actix/examples/tree/master/websockets/chat-actorless)
- Minimum Supported Rust Version (MSRV): 1.75

## Usage

```toml
# Cargo.toml
anyhow = "1"
actix-web = "4"
actix-ws-ng = "0.3"
```

```rust
// main.rs
use actix_web::{middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_ws::Message;

async fn ws(req: HttpRequest, body: web::Payload) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        return;
                    }
                }
                Message::Text(s) => println!("Got text, {}", s),
                _ => break,
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/ws", web::get().to(ws))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}
```

## License

This project is licensed under either of

- Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.
