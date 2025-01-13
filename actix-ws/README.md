# `actix-ws`

> WebSockets for Actix Web, without actors.

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-ws?label=latest)](https://crates.io/crates/actix-ws)
[![Documentation](https://docs.rs/actix-ws/badge.svg?version=0.3.0)](https://docs.rs/actix-ws/0.3.0)
![Version](https://img.shields.io/badge/rustc-1.75+-ab6000.svg)
![MIT or Apache 2.0 licensed](https://img.shields.io/crates/l/actix-ws.svg)
<br />
[![Dependency Status](https://deps.rs/crate/actix-ws/0.3.0/status.svg)](https://deps.rs/crate/actix-ws/0.3.0)
[![Download](https://img.shields.io/crates/d/actix-ws.svg)](https://crates.io/crates/actix-ws)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/NWpN5mmg3x)

<!-- prettier-ignore-end -->

## Example

```rust
use actix_web::{middleware::Logger, web, App, HttpRequest, HttpServer, Responder};
use actix_ws::Message;

async fn ws(req: HttpRequest, body: web::Payload) -> actix_web::Result<impl Responder> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.recv().await {
            match msg {
                Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        return;
                    }
                }
                Message::Text(msg) => println!("Got text: {msg}"),
                _ => break,
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
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

## Resources

- [API Documentation](https://docs.rs/actix-ws)
- [Example Chat Project](https://github.com/actix/examples/tree/master/websockets/chat-actorless)
- Minimum Supported Rust Version (MSRV): 1.75

## License

This project is licensed under either of

- Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.
