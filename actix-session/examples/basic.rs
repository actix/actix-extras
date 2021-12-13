use actix_session::{storage::RedisActorSessionStore, Session, SessionMiddleware};
use actix_web::cookie::Key;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpServer, Responder};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

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

// The signing key would usually be read from a configuration file/environment variables.
fn get_signing_key() -> Key {
    let signing_key: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    Key::from(signing_key.as_bytes())
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info,actix_redis=info");
    env_logger::init();
    let signing_key = get_signing_key();

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
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
