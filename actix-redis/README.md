# actix-redis

[![crates.io](https://img.shields.io/crates/v/actix-redis)](https://crates.io/crates/actix-redis)
[![Documentation](https://docs.rs/actix-redis/badge.svg)](https://docs.rs/actix-redis)
[![Dependency Status](https://deps.rs/crate/actix-redis/0.8.1/status.svg)](https://deps.rs/crate/actix-redis/0.8.1)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-redis)
[![Join the chat at https://gitter.im/actix/actix](https://badges.gitter.im/actix/actix.svg)](https://gitter.im/actix/actix?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

> Redis integration for actix framework.

## Documentation

* [API Documentation](https://actix.rs/actix-extras/actix_redis/)
* [Chat on gitter](https://gitter.im/actix/actix)
* Cargo package: [actix-redis](https://crates.io/crates/actix-redis)
* Minimum supported Rust version: 1.40 or later

## Redis session backend

Use redis as session storage.

You need to pass an address of the redis server and random value to the
constructor of `RedisSession`. This is private key for cookie session,
When this value is changed, all session data is lost.

Note that whatever you write into your session is visible by the user (but not modifiable).

Constructor panics if key length is less than 32 bytes.

```rust
use actix_web::{App, HttpServer, web, middleware};
use actix_web::middleware::session::SessionStorage;
use actix_redis::RedisSession;

#[actix_rt::main]
async fn main() -> std::io::Result {
    HttpServer::new(|| App::new()
        // enable logger
        .middleware(middleware::Logger::default())
        // cookie session middleware
        .middleware(SessionStorage::new(
            RedisSession::new("127.0.0.1:6379", &[0; 32])
        ))
        // register simple route, handle all methods
        .service(web::resource("/").to(index))
    )
    .bind("0.0.0.0:8080")?
    .start()
    .await
}
```

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))

at your option.

## Code of Conduct

Contribution to the actix-redis crate is organized under the terms of the
Contributor Covenant, the maintainer of actix-redis, @fafhrd91, promises to
intervene to uphold that code of conduct.
