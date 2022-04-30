use crate::Identity;
use actix_web::dev::ServiceRequest;
use actix_web::guard::GuardContext;
use actix_web::HttpMessage;
use actix_web::HttpRequest;

/// Helper trait to retrieve an [`Identity`] instance from various `actix-web`'s types.
pub trait IdentityExt {
    fn get_identity(&self) -> Identity;
}

impl IdentityExt for HttpRequest {
    fn get_identity(&self) -> Identity {
        Identity::extract(&self.extensions())
    }
}

impl IdentityExt for ServiceRequest {
    fn get_identity(&self) -> Identity {
        Identity::extract(&self.extensions())
    }
}

impl<'a> IdentityExt for GuardContext<'a> {
    fn get_identity(&self) -> Identity {
        Identity::extract(&self.req_data())
    }
}
