use actix_session::{storage::RedisSessionStore, Session, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, Error, HttpRequest, HttpServer, Responder};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// simple handler
async fn index(req: HttpRequest, session: Session) -> Result<impl Responder, Error> {
    println!("{req:?}");

    // session
    if let Some(count) = session.get::<i32>("counter")? {
        println!("SESSION value: {count}");
        session.insert("counter", count + 1)?;
    } else {
        session.insert("counter", 1)?;
    }

    Ok("Welcome!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    // The signing key would usually be read from a configuration file/environment variables.
    let signing_key = Key::generate();

    tracing::info!("setting up Redis session storage");
    let storage = RedisSessionStore::new("127.0.0.1:6379").await.unwrap();

    tracing::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            // cookie session middleware
            .wrap(SessionMiddleware::new(storage.clone(), signing_key.clone()))
            // register simple route, handle all methods
            .service(web::resource("/").to(index))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
