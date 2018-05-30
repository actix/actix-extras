extern crate actix_web;
extern crate actix_web_httpauth;

use actix_web::{server, App, HttpRequest, HttpResponse};
use actix_web::http::StatusCode;
use actix_web_httpauth::headers::www_authenticate::{WWWAuthenticate};
use actix_web_httpauth::headers::www_authenticate::basic::Basic;


fn index(req: HttpRequest) -> HttpResponse {
    let challenge = Basic {
        realm: Some("Restricted area".to_string()),
    };

    req.build_response(StatusCode::UNAUTHORIZED)
        .set(WWWAuthenticate(challenge))
        .finish()
}

fn main() {
    server::new(|| App::new()
            .resource("/", |r| r.with(index)))
       .bind("127.0.0.1:8088").unwrap()
       .run();
}
