use actix_session::{storage::SqliteSessionStore, Session, SessionMiddleware};
use actix_web::{
    cookie::{time::Duration, Key},
    middleware, web, App, Error, HttpRequest, HttpServer, Responder,
};
use r2d2_sqlite::{self, SqliteConnectionManager};

/// simple handler
async fn index(_req: HttpRequest, session: Session) -> Result<impl Responder, Error> {
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

    let manager = SqliteConnectionManager::file("sessions.db");
    let pool = r2d2::Pool::<r2d2_sqlite::SqliteConnectionManager>::new(manager).unwrap();

    let sqlite_session_store = SqliteSessionStore::new(pool, true).unwrap();

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            // enable logger
            // cookie session middleware
            .wrap(
                SessionMiddleware::builder(sqlite_session_store.clone(), signing_key.clone())
                    .cookie_name(String::from("session"))
                    .cookie_secure(false)
                    .session_lifecycle(
                        actix_session::config::PersistentSession::default()
                            .session_ttl(Duration::seconds(100)),
                    )
                    .build(),
            )
            .wrap(middleware::Logger::default())
            // register simple route, handle all methods
            .service(web::resource("/").to(index))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
