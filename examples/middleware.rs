use actix_web::dev::ServiceRequest;
use actix_web::{middleware, web, App, Error, HttpServer};

use futures::future;

use actix_web_httpauth::extractors::basic::BasicAuth;
use actix_web_httpauth::middleware::HttpAuthentication;

fn validator(
    req: ServiceRequest,
    _credentials: BasicAuth,
) -> future::FutureResult<ServiceRequest, Error> {
    future::ok(req)
}

fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let auth = HttpAuthentication::basic(validator);
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(auth)
            .service(web::resource("/").to(|| "Test\r\n"))
    })
    .bind("127.0.0.1:8080")?
    .workers(1)
    .run()
}
