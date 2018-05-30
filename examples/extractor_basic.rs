extern crate actix_web;
extern crate actix_web_httpauth;

use actix_web::{server, App, Result, HttpRequest, FromRequest};
use actix_web::middleware::{Middleware, Started};
use actix_web_httpauth::extractors::basic::{BasicAuth, Config};
use actix_web_httpauth::extractors::AuthenticationError;

struct Auth;

impl<S> Middleware<S> for Auth {
    fn start(&self, req: &mut HttpRequest<S>) -> Result<Started> {
        let mut config = Config::default();
        config.realm("WallyWorld");
        let auth = BasicAuth::from_request(&req, &config)?;

        if auth.username() == "Aladdin" && auth.password() == Some("open sesame") {
            Ok(Started::Done)
        } else {
            Err(AuthenticationError::from(config).into())
        }
    }
}

fn index(_req: HttpRequest) -> String {
    "Hello, authorized user!".to_string()
}

fn main() {
    server::new(|| App::new()
            .middleware(Auth)
            .resource("/", |r| r.with(index))
        )
        .bind("127.0.0.1:8088").unwrap()
        .run();
}
