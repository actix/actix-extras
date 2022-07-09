use std::net::TcpListener;

use actix_identity::{config::IdentityMiddlewareBuilder, Identity, IdentityMiddleware};
use actix_session::{Session, SessionStatus};
use actix_web::{web, App, HttpMessage, HttpRequest, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};

use crate::fixtures::session_middleware;

pub struct TestApp {
    port: u16,
    api_client: reqwest::Client,
}

impl TestApp {
    /// Spawn a test application using a custom configuration for `IdentityMiddleware`.
    pub fn spawn_with_config(builder: IdentityMiddlewareBuilder) -> Self {
        // Random OS port
        let listener = TcpListener::bind("localhost:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = HttpServer::new(move || {
            App::new()
                .wrap(builder.clone().build())
                .wrap(session_middleware())
                .route("/increment", web::post().to(increment))
                .route("/current", web::get().to(show))
                .route("/login", web::post().to(login))
                .route("/logout", web::post().to(logout))
                .route("/identity_required", web::get().to(identity_required))
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        let _ = actix_web::rt::spawn(server);

        let client = reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();

        TestApp {
            port,
            api_client: client,
        }
    }

    /// Spawn a test application using the default configuration settings for `IdentityMiddleware`.
    pub fn spawn() -> Self {
        Self::spawn_with_config(IdentityMiddleware::builder())
    }

    fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    pub async fn get_identity_required(&self) -> reqwest::Response {
        self.api_client
            .get(format!("{}/identity_required", &self.url()))
            .send()
            .await
            .unwrap()
    }

    pub async fn get_current(&self) -> EndpointResponse {
        self.api_client
            .get(format!("{}/current", &self.url()))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn post_increment(&self) -> EndpointResponse {
        let response = self
            .api_client
            .post(format!("{}/increment", &self.url()))
            .send()
            .await
            .unwrap();
        response.json().await.unwrap()
    }

    pub async fn post_login(&self, user_id: String) -> EndpointResponse {
        let response = self
            .api_client
            .post(format!("{}/login", &self.url()))
            .json(&LoginRequest { user_id })
            .send()
            .await
            .unwrap();
        response.json().await.unwrap()
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(format!("{}/logout", &self.url()))
            .send()
            .await
            .unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct EndpointResponse {
    pub user_id: Option<String>,
    pub counter: i32,
    pub session_status: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct LoginRequest {
    user_id: String,
}

async fn show(user: Option<Identity>, session: Session) -> HttpResponse {
    let user_id = user.map(|u| u.id().unwrap());
    let counter: i32 = session
        .get::<i32>("counter")
        .unwrap_or(Some(0))
        .unwrap_or(0);

    HttpResponse::Ok().json(&EndpointResponse {
        user_id,
        counter,
        session_status: session_status(session),
    })
}

async fn increment(session: Session, user: Option<Identity>) -> HttpResponse {
    let user_id = user.map(|u| u.id().unwrap());
    let counter: i32 = session
        .get::<i32>("counter")
        .unwrap_or(Some(0))
        .map_or(1, |inner| inner + 1);
    session.insert("counter", &counter).unwrap();

    HttpResponse::Ok().json(&EndpointResponse {
        user_id,
        counter,
        session_status: session_status(session),
    })
}

async fn login(
    user_id: web::Json<LoginRequest>,
    request: HttpRequest,
    session: Session,
) -> HttpResponse {
    let id = user_id.into_inner().user_id;
    let user = Identity::login(&request.extensions(), id).unwrap();

    let counter: i32 = session
        .get::<i32>("counter")
        .unwrap_or(Some(0))
        .unwrap_or(0);

    HttpResponse::Ok().json(&EndpointResponse {
        user_id: Some(user.id().unwrap()),
        counter,
        session_status: session_status(session),
    })
}

async fn logout(user: Option<Identity>) -> HttpResponse {
    if let Some(user) = user {
        user.logout();
    }
    HttpResponse::Ok().finish()
}

async fn identity_required(_identity: Identity) -> HttpResponse {
    HttpResponse::Ok().finish()
}

fn session_status(session: Session) -> String {
    match session.status() {
        SessionStatus::Changed => "changed",
        SessionStatus::Purged => "purged",
        SessionStatus::Renewed => "renewed",
        SessionStatus::Unchanged => "unchanged",
    }
    .into()
}
