# actix-identity

> Identity management for Actix Web.

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-identity?label=latest)](https://crates.io/crates/actix-identity)
[![Documentation](https://docs.rs/actix-identity/badge.svg?version=0.7.0)](https://docs.rs/actix-identity/0.7.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-identity)
[![Dependency Status](https://deps.rs/crate/actix-identity/0.7.0/status.svg)](https://deps.rs/crate/actix-identity/0.7.0)

<!-- prettier-ignore-end -->

<!-- cargo-rdme start -->

Identity management for Actix Web.

`actix-identity` can be used to track identity of a user across multiple requests. It is built on top of HTTP sessions, via [`actix-session`](https://docs.rs/actix-session).

## Getting started

To start using identity management in your Actix Web application you must register [`IdentityMiddleware`] and `SessionMiddleware` as middleware on your `App`:

```rust
use actix_web::{cookie::Key, App, HttpServer, HttpResponse};
use actix_identity::IdentityMiddleware;
use actix_session::{storage::RedisSessionStore, SessionMiddleware};

#[actix_web::main]
async fn main() {
    // When using `Key::generate()` it is important to initialize outside of the
    // `HttpServer::new` closure. When deployed the secret key should be read from a
    // configuration file or environment variables.
    let secret_key = Key::generate();

    let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379")
        .await
        .unwrap();

    HttpServer::new(move || {
        App::new()
            // Install the identity framework first.
            .wrap(IdentityMiddleware::default())
            // The identity system is built on top of sessions. You must install the session
            // middleware to leverage `actix-identity`. The session middleware must be mounted
            // AFTER the identity middleware: `actix-web` invokes middleware in the OPPOSITE
            // order of registration when it receives an incoming request.
            .wrap(SessionMiddleware::new(
                 redis_store.clone(),
                 secret_key.clone()
            ))
            // Your request handlers [...]
    })
}
```

User identities can be created, accessed and destroyed using the [`Identity`] extractor in your request handlers:

```rust
use actix_web::{get, post, HttpResponse, Responder, HttpRequest, HttpMessage};
use actix_identity::Identity;
use actix_session::storage::RedisSessionStore;

#[get("/")]
async fn index(user: Option<Identity>) -> impl Responder {
    if let Some(user) = user {
        format!("Welcome! {}", user.id().unwrap())
    } else {
        "Welcome Anonymous!".to_owned()
    }
}

#[post("/login")]
async fn login(request: HttpRequest) -> impl Responder {
    // Some kind of authentication should happen here
    // e.g. password-based, biometric, etc.
    // [...]

    // attach a verified user identity to the active session
    Identity::login(&request.extensions(), "User1".into()).unwrap();

    HttpResponse::Ok()
}

#[post("/logout")]
async fn logout(user: Identity) -> impl Responder {
    user.logout();
    HttpResponse::Ok()
}
```

## Advanced configuration

By default, `actix-identity` does not automatically log out users. You can change this behaviour by customising the configuration for [`IdentityMiddleware`] via [`IdentityMiddleware::builder`].

In particular, you can automatically log out users who:

- have been inactive for a while (see [`IdentityMiddlewareBuilder::visit_deadline`];
- logged in too long ago (see [`IdentityMiddlewareBuilder::login_deadline`]).

[`IdentityMiddlewareBuilder::visit_deadline`]: config::IdentityMiddlewareBuilder::visit_deadline
[`IdentityMiddlewareBuilder::login_deadline`]: config::IdentityMiddlewareBuilder::login_deadline

<!-- cargo-rdme end -->
