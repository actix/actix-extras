use actix_utils::future::{ready, Ready};
use actix_web::{
    dev::{Payload, ServiceRequest, ServiceResponse},
    error::Error,
    FromRequest, HttpMessage, HttpRequest,
};

use crate::Session;

/// Extract a [`Session`] object from various `actix-web` types (e.g. `HttpRequest`, `ServiceRequest`, `ServiceResponse`).
pub trait SessionExt {
    /// Extract a [`Session`] object.
    fn get_session(&self) -> Session;
}

impl SessionExt for HttpRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

impl SessionExt for ServiceRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

impl SessionExt for ServiceResponse {
    fn get_session(&self) -> Session {
        self.request().get_session()
    }
}

/// Extractor implementation for [`Session`]s.
///
/// # Examples
/// ```
/// # use actix_web::*;
/// use actix_session::Session;
///
/// #[get("/")]
/// async fn index(session: Session) -> Result<impl Responder> {
///     // access session data
///     if let Some(count) = session.get::<i32>("counter")? {
///         session.insert("counter", count + 1)?;
///     } else {
///         session.insert("counter", 1)?;
///     }
///
///     let count = session.get::<i32>("counter")?.unwrap();
///     Ok(format!("Counter: {}", count))
/// }
/// ```
impl FromRequest for Session {
    type Error = Error;
    type Future = Ready<Result<Session, Error>>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(Ok(Session::get_session(&mut *req.extensions_mut())))
    }
}
