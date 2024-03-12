# actix-session

> Session management for Actix Web.

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-session?label=latest)](https://crates.io/crates/actix-session)
[![Documentation](https://docs.rs/actix-session/badge.svg?version=0.9.0)](https://docs.rs/actix-session/0.9.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-session)
[![Dependency Status](https://deps.rs/crate/actix-session/0.9.0/status.svg)](https://deps.rs/crate/actix-session/0.9.0)

<!-- prettier-ignore-end -->

<!-- cargo-rdme start -->

Session management for Actix Web.

The HTTP protocol, at a first glance, is stateless: the client sends a request, the server parses its content, performs some processing and returns a response. The outcome is only influenced by the provided inputs (i.e. the request content) and whatever state the server queries while performing its processing.

Stateless systems are easier to reason about, but they are not quite as powerful as we need them to be - e.g. how do you authenticate a user? The user would be forced to authenticate **for every single request**. That is, for example, how 'Basic' Authentication works. While it may work for a machine user (i.e. an API client), it is impractical for a person—you do not want a login prompt on every single page you navigate to!

There is a solution - **sessions**. Using sessions the server can attach state to a set of requests coming from the same client. They are built on top of cookies - the server sets a cookie in the HTTP response (`Set-Cookie` header), the client (e.g. the browser) will store the cookie and play it back to the server when sending new requests (using the `Cookie` header).

We refer to the cookie used for sessions as a **session cookie**. Its content is called **session key** (or **session ID**), while the state attached to the session is referred to as **session state**.

`actix-session` provides an easy-to-use framework to manage sessions in applications built on top of Actix Web. [`SessionMiddleware`] is the middleware underpinning the functionality provided by `actix-session`; it takes care of all the session cookie handling and instructs the **storage backend** to create/delete/update the session state based on the operations performed against the active [`Session`].

`actix-session` provides some built-in storage backends: ([`CookieSessionStore`], [`RedisSessionStore`], and [`RedisActorSessionStore`]) - you can create a custom storage backend by implementing the [`SessionStore`] trait.

Further reading on sessions:

- [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265);
- [OWASP's session management cheat-sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).

## Getting started

To start using sessions in your Actix Web application you must register [`SessionMiddleware`] as a middleware on your `App`:

```rust
use actix_web::{web, App, HttpServer, HttpResponse, Error};
use actix_session::{Session, SessionMiddleware, storage::RedisSessionStore};
use actix_web::cookie::Key;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // When using `Key::generate()` it is important to initialize outside of the
    // `HttpServer::new` closure. When deployed the secret key should be read from a
    // configuration file or environment variables.
    let secret_key = Key::generate();

    let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379")
        .await
        .unwrap();

    HttpServer::new(move ||
            App::new()
            // Add session management to your application using Redis for session state storage
            .wrap(
                SessionMiddleware::new(
                    redis_store.clone(),
                    secret_key.clone(),
                )
            )
            .default_service(web::to(|| HttpResponse::Ok())))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
```

The session state can be accessed and modified by your request handlers using the [`Session`] extractor. Note that this doesn't work in the stream of a streaming response.

```rust
use actix_web::Error;
use actix_session::Session;

fn index(session: Session) -> Result<&'static str, Error> {
    // access the session state
    if let Some(count) = session.get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        // modify the session state
        session.insert("counter", count + 1)?;
    } else {
        session.insert("counter", 1)?;
    }

    Ok("Welcome!")
}
```

## Choosing A Backend

By default, `actix-session` does not provide any storage backend to retrieve and save the state attached to your sessions. You can enable:

- a purely cookie-based "backend", [`CookieSessionStore`], using the `cookie-session` feature flag.

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["cookie-session"] }
  ```

- a Redis-based backend via [`redis-rs`](https://docs.rs/redis-rs), [`RedisSessionStore`], using the `redis-rs-session` feature flag.

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["redis-rs-session"] }
  ```

  Add the `redis-rs-tls-session` feature flag if you want to connect to Redis using a secured connection:

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["redis-rs-session", "redis-rs-tls-session"] }
  ```

You can implement your own session storage backend using the [`SessionStore`] trait.

[`SessionStore`]: storage::SessionStore
[`CookieSessionStore`]: storage::CookieSessionStore
[`RedisSessionStore`]: storage::RedisSessionStore
[`RedisActorSessionStore`]: storage::RedisActorSessionStore

<!-- cargo-rdme end -->
