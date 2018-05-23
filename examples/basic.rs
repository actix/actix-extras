extern crate actix_web;
extern crate actix_web_httpauth;

use actix_web::{server, App, HttpRequest, FromRequest, Result};
use actix_web::middleware::{Middleware, Started};
use actix_web_httpauth::basic::{BasicAuth, Config};

struct AuthMiddleware;

impl<S> Middleware<S> for AuthMiddleware {
    fn start(&self, req: &mut HttpRequest<S>) -> Result<Started> {
        let mut config = Config::default();
        config.realm("Restricted area".to_string());
        let auth = BasicAuth::from_request(&req, &config)?;

        // Please note that this is only an example,
        // do not ever hardcode your credentials!
        if auth.username == "root" && auth.password == "pass" {
            Ok(Started::Done)
        } else {
            let response = BasicAuth::error_response(&config);
            Ok(Started::Response(response))
        }
    }

}

fn index(auth: BasicAuth) -> String {
    format!("Hello, {}", auth.username)
}

fn main() {
    server::new(|| App::new()
            // Comment the `.middleware()` line and let `BasicAuth` extractor
            // in the `index` handler do the authentication routine
            .middleware(AuthMiddleware)
            .resource("/", |r| r.with(index)))
       .bind("127.0.0.1:8088").unwrap()
       .run();
}
