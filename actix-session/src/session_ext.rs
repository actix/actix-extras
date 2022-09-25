use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    guard::GuardContext,
    HttpMessage, HttpRequest,
};

use crate::Session;

/// Extract a [`Session`] object from various `actix-web` types (e.g. `HttpRequest`,
/// `ServiceRequest`, `ServiceResponse`).
pub trait SessionExt {
    /// Extract a [`Session`] object.
    fn get_session(&self) -> Session;
}

impl SessionExt for HttpRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut self.extensions_mut())
    }
}

impl SessionExt for ServiceRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut self.extensions_mut())
    }
}

impl SessionExt for ServiceResponse {
    fn get_session(&self) -> Session {
        self.request().get_session()
    }
}

impl<'a> SessionExt for GuardContext<'a> {
    fn get_session(&self) -> Session {
        Session::get_session(&mut self.req_data_mut())
    }
}
