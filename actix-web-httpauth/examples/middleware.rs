use actix_web::{
    dev::ServiceRequest, error, get, middleware::Logger, App, Error, HttpServer, Responder,
};
use actix_web_httpauth::{extractors::bearer::BearerAuth, middleware::HttpAuthentication};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// Validator that:
/// - accepts Bearer auth;
/// - returns a custom response for requests without a valid Bearer Authorization header;
/// - rejects tokens containing an "x" (for quick testing using command line HTTP clients).
async fn validator(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let Some(credentials) = credentials else {
        return Err((error::ErrorBadRequest("no bearer header"), req));
    };

    eprintln!("{credentials:?}");

    if credentials.token().contains('x') {
        return Err((error::ErrorBadRequest("token contains x"), req));
    }

    Ok(req)
}

#[get("/")]
async fn index(auth: BearerAuth) -> impl Responder {
    format!("authenticated for token: {}", auth.token().to_owned())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .without_time()
        .init();

    HttpServer::new(|| {
        let auth = HttpAuthentication::with_fn(validator);

        App::new()
            .service(index)
            .wrap(auth)
            .wrap(Logger::default().log_target("@"))
    })
    .bind("127.0.0.1:8080")?
    .workers(2)
    .run()
    .await
}
