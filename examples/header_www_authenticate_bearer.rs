extern crate actix_web;
extern crate actix_web_httpauth;

use actix_web::{server, App, HttpRequest, HttpResponse};
use actix_web::http::StatusCode;
use actix_web_httpauth::headers::www_authenticate::{WWWAuthenticate};
use actix_web_httpauth::headers::www_authenticate::bearer::{Bearer, Error};


fn index(req: HttpRequest) -> HttpResponse {
    let challenge = Bearer {
        realm: Some("example".to_string()),
        scope: Some("openid profile email".to_string()),
        error: Some(Error::InvalidToken),
        error_description: Some("The access token expired".to_string()),
        error_uri: Some("http://example.org".to_string()),
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
