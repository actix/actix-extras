use actix_redis::RedisSession;
use actix_session::Session;
use actix_web::{
    cookie, error::InternalError, middleware, web, App, Error, HttpResponse, HttpServer,
    Responder,
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
    fn authenticate(credentials: Credentials) -> Result<Self, Error> {
        // TODO: figure out why I keep getting hacked
        if &credentials.password != "hunter2" {
            return Err(unauthorized());
        }

        Ok(User {
            id: 42,
            username: credentials.username,
            password: credentials.password,
        })
    }
}

fn unauthorized() -> Error {
    InternalError::from_response(
        "Unauthorized",
        HttpResponse::Unauthorized().json("Unauthorized").into(),
    )
    .into()
}

pub fn validate_session(session: &Session) -> Result<i64, Error> {
    let user_id: i64 = session
        .get("user_id")
        .unwrap_or(None)
        .ok_or_else(unauthorized)?;
    // keep the user's session alive
    session.renew();
    Ok(user_id)
}

async fn login(
    credentials: web::Json<Credentials>,
    session: Session,
) -> Result<impl Responder, Error> {
    let credentials = credentials.into_inner();

    let user = User::authenticate(credentials)?;
    session.insert("user_id", user.id).unwrap();

    Ok("Welcome!")
}

/// some protected resource
async fn secret(session: Session) -> Result<impl Responder, Error> {
    // only allow access to this resource if the user has an active session
    validate_session(&session)?;

    Ok("secret revealed")
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info,actix_redis=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            // cookie session middleware
            .wrap(
                RedisSession::new("127.0.0.1:6379", &[0; 32])
                    // allow the cookie to be accessed from javascript
                    .cookie_http_only(false)
                    // allow the cookie only from the current domain
                    .cookie_same_site(cookie::SameSite::Strict),
            )
            .route("/login", web::post().to(login))
            .route("/secret", web::get().to(secret))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
