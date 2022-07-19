use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::Key;
use uuid::Uuid;

pub fn store() -> CookieSessionStore {
    CookieSessionStore::default()
}

pub fn user_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn session_middleware() -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(store(), Key::generate())
        .cookie_domain(Some("localhost".into()))
        .build()
}
