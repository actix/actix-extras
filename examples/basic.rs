extern crate actix_web;
extern crate actix_web_httpauth;

use actix_web::http::StatusCode;
use actix_web::{server, App, HttpRequest, FromRequest, Result};
use actix_web::middleware::{Middleware, Started};
use actix_web_httpauth::BasicAuth;

struct AuthMiddleware;

impl<S> Middleware<S> for AuthMiddleware {
    fn start(&self, req: &mut HttpRequest<S>) -> Result<Started> {
        let auth = BasicAuth::extract(&req)?;

        // Please note that this is only an example,
        // do not ever hardcode your credentials!
        if auth.username == "root" && auth.password == "pass" {
            Ok(Started::Done)
        } else {
            let response = req.build_response(StatusCode::UNAUTHORIZED)
                .header("WWW-Authenticate", "Basic")
                .finish();
            Ok(Started::Response(response))
        }
    }

}

fn index(auth: BasicAuth) -> String {
    format!("Hello, {}", auth.username)
}

fn main() {
    server::new(|| App::new()
            // Comment line below to pass authentication handling
            // directly to `index` handler.
            .middleware(AuthMiddleware)
            .resource("/", |r| r.with(index)))
       .bind("127.0.0.1:8088").unwrap()
       .run();
}
