# actix-redis

> Redis integration for Actix and session store for Actix Web.

[![crates.io](https://img.shields.io/crates/v/actix-redis?label=latest)](https://crates.io/crates/actix-redis)
[![Documentation](https://docs.rs/actix-redis/badge.svg?version=0.10.0-beta.2)](https://docs.rs/actix-redis/0.10.0-beta.2)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-redis)
[![Dependency Status](https://deps.rs/crate/actix-redis/0.10.0-beta.2/status.svg)](https://deps.rs/crate/actix-redis/0.10.0-beta.2)

## Documentation & Resources

- [API Documentation](https://docs.rs/actix-redis)
- [Example Project](https://github.com/actix/examples/tree/HEAD/session/redis-session)
- Minimum Supported Rust Version (MSRV): 1.51

## Redis Session Backend

Use redis as session storage.

You need to pass an address of the redis server and random value to the
constructor of `RedisSession`. This is private key for cookie session,
When this value is changed, all session data is lost.

Note that whatever you write into your session is visible by the user (but not modifiable).

Constructor panics if key length is less than 32 bytes.

```rust
use actix_web::{App, HttpServer, middleware::Logger};
use actix_web::web::{resource, get}
use actix_redis::RedisSession;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || App::new()
        // cookie session middleware
        .wrap(RedisSession::new("127.0.0.1:6379", &[0; 32]))
        // enable logger
        .wrap(Logger::default())
        // register simple route, handle all methods
        .service(resource("/").route(get().to(index)))
    )
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```
