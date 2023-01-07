use actix_cors::Cors;
use actix_web::{dev::ServiceRequest, get, App, Error, HttpResponse, HttpServer};
use actix_web_httpauth::{extractors::bearer::BearerAuth, middleware::HttpAuthentication};

async fn ok_validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    eprintln!("{credentials:?}");
    Ok(req)
}

#[get("/")]
async fn index() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .wrap(HttpAuthentication::bearer(ok_validator))
            // ensure the CORS middleware is wrapped around the httpauth middleware so it is able to
            // add headers to error responses
            .wrap(Cors::permissive())
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
