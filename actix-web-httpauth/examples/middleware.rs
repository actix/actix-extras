use actix_web::{dev::ServiceRequest, middleware, web, App, Error, HttpServer};
use actix_web_httpauth::{extractors::basic::BasicAuth, middleware::HttpAuthentication};

async fn validator(
    req: ServiceRequest,
    _credentials: BasicAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    Ok(req)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let auth = HttpAuthentication::basic(validator);
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(auth)
            .service(web::resource("/").to(|| async { "Test\r\n" }))
    })
    .bind("127.0.0.1:8080")?
    .workers(1)
    .run()
    .await
}
