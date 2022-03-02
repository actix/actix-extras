use actix_session::{storage::RedisActorSessionStore, Session, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, Error, HttpRequest, HttpServer, Responder};

/// simple handler
async fn index(req: HttpRequest, session: Session) -> Result<impl Responder, Error> {
    println!("{:?}", req);

    // session
    if let Some(count) = session.get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        session.insert("counter", count + 1)?;
    } else {
        session.insert("counter", 1)?;
    }

    Ok("Welcome!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // The signing key would usually be read from a configuration file/environment variables.
    let signing_key = Key::generate();

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            // cookie session middleware
            .wrap(SessionMiddleware::new(
                RedisActorSessionStore::new("127.0.0.1:6379"),
                signing_key.clone(),
            ))
            // register simple route, handle all methods
            .service(web::resource("/").to(index))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
