extern crate actix_web;
extern crate actix_web_httpauth;

use actix_web::{server, App, HttpRequest, Result, FromRequest};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::extractors::bearer::{BearerAuth, Config, Error};
use actix_web::middleware::{Middleware, Started};

struct Auth;

impl<S> Middleware<S> for Auth {
    fn start(&self, req: &HttpRequest<S>) -> Result<Started> {
        let mut config = Config::default();
        config.realm("Restricted area");
        config.scope("openid profile email");
        let auth = BearerAuth::from_request(&req, &config)?;

        if auth.token() == "mF_9.B5f-4.1JqM" {
            Ok(Started::Done)
        } else {
            Err(AuthenticationError::from(config)
                .with_error(Error::InvalidToken)
                .into())
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
