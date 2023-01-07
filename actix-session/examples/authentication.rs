use actix_session::{storage::RedisActorSessionStore, Session, SessionMiddleware};
use actix_web::{
    cookie::{Key, SameSite},
    error::InternalError,
    middleware, web, App, Error, HttpResponse, HttpServer, Responder,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct User {
    id: i64,
    username: String,
    password: String,
}

impl User {
    fn authenticate(credentials: Credentials) -> Result<Self, HttpResponse> {
        // to do: figure out why I keep getting hacked      /s
        if &credentials.password != "hunter2" {
            return Err(HttpResponse::Unauthorized().json("Unauthorized"));
        }

        Ok(User {
            id: 42,
            username: credentials.username,
            password: credentials.password,
        })
    }
}

pub fn validate_session(session: &Session) -> Result<i64, HttpResponse> {
    let user_id: Option<i64> = session.get("user_id").unwrap_or(None);

    match user_id {
        Some(id) => {
            // keep the user's session alive
            session.renew();
            Ok(id)
        }
        None => Err(HttpResponse::Unauthorized().json("Unauthorized")),
    }
}

async fn login(
    credentials: web::Json<Credentials>,
    session: Session,
) -> Result<impl Responder, Error> {
    let credentials = credentials.into_inner();

    match User::authenticate(credentials) {
        Ok(user) => session.insert("user_id", user.id).unwrap(),
        Err(err) => return Err(InternalError::from_response("", err).into()),
    };

    Ok("Welcome!")
}

/// some protected resource
async fn secret(session: Session) -> Result<impl Responder, Error> {
    // only allow access to this resource if the user has an active session
    validate_session(&session).map_err(|err| InternalError::from_response("", err))?;

    Ok("secret revealed")
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
            .wrap(
                SessionMiddleware::builder(
                    RedisActorSessionStore::new("127.0.0.1:6379"),
                    signing_key.clone(),
                )
                // allow the cookie to be accessed from javascript
                .cookie_http_only(false)
                // allow the cookie only from the current domain
                .cookie_same_site(SameSite::Strict)
                .build(),
            )
            .route("/login", web::post().to(login))
            .route("/secret", web::get().to(secret))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
