//! A rudimentary example of how to set up and use `actix-identity`.
//!
//! ```bash
//! # using HTTPie (https://httpie.io/cli)
//!
//! # outputs "Welcome Anonymous!" message
//! http -v --session=identity GET localhost:8080/
//!
//! # log in using fake details, ensuring that --session is used to persist cookies
//! http -v --session=identity POST localhost:8080/login user_id=foo
//!
//! # outputs "Welcome User1" message
//! http -v --session=identity GET localhost:8080/
//! ```

use std::io;

use actix_identity::{Identity, IdentityMiddleware};
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{
    cookie::Key, get, middleware::Logger, post, App, HttpMessage, HttpRequest, HttpResponse,
    HttpServer, Responder,
};

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let secret_key = Key::generate();

    HttpServer::new(move || {
        let session_mw =
            SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                // disable secure cookie for local testing
                .cookie_secure(false)
                .build();

        App::new()
            // Install the identity framework first.
            .wrap(IdentityMiddleware::default())
            // The identity system is built on top of sessions. You must install the session
            // middleware to leverage `actix-identity`. The session middleware must be mounted
            // AFTER the identity middleware: `actix-web` invokes middleware in the OPPOSITE
            // order of registration when it receives an incoming request.
            .wrap(session_mw)
            .wrap(Logger::default())
            .service(index)
            .service(login)
            .service(logout)
    })
    .bind(("127.0.0.1", 8080))
    .unwrap()
    .workers(2)
    .run()
    .await
}

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
    // Some kind of authentication should happen here -
    // e.g. password-based, biometric, etc.
    // [...]

    // Attached a verified user identity to the active
    // session.
    Identity::login(&request.extensions(), "User1".into()).unwrap();

    HttpResponse::Ok()
}

#[post("/logout")]
async fn logout(user: Identity) -> impl Responder {
    user.logout();
    HttpResponse::NoContent()
}
