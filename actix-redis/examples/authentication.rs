use actix_redis::RedisSession;
use actix_session::Session;
use actix_web::{
    cookie, middleware, web, App, Error, HttpResponse, HttpServer, Responder,
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
        // TODO: figure out why I keep getting hacked
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
) -> Result<impl Responder, HttpResponse> {
    let credentials = credentials.into_inner();

    match User::authenticate(credentials) {
        Ok(user) => session.set("user_id", user.id).unwrap(),
        Err(_) => return Err(HttpResponse::Unauthorized().json("Unauthorized")),
    };

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
