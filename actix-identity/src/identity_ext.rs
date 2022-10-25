use actix_web::{dev::ServiceRequest, guard::GuardContext, HttpMessage, HttpRequest};

use crate::{Identity, IdentityError};

/// Helper trait to retrieve an [`Identity`] instance from various `actix-web`'s types.
pub trait IdentityExt {
    /// Retrieve the identity attached to the current session, if available.
    fn get_identity(&self) -> Result<Identity, IdentityError>;
}

impl IdentityExt for HttpRequest {
    fn get_identity(&self) -> Result<Identity, IdentityError> {
        Identity::extract(&self.extensions())
    }
}

impl IdentityExt for ServiceRequest {
    fn get_identity(&self) -> Result<Identity, IdentityError> {
        Identity::extract(&self.extensions())
    }
}

impl<'a> IdentityExt for GuardContext<'a> {
    fn get_identity(&self) -> Result<Identity, IdentityError> {
        Identity::extract(&self.req_data())
    }
}
