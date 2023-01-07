use actix_web::{dev::ServiceRequest, guard::GuardContext, HttpMessage, HttpRequest};

use crate::{error::GetIdentityError, Identity};

/// Helper trait to retrieve an [`Identity`] instance from various `actix-web`'s types.
pub trait IdentityExt {
    /// Retrieve the identity attached to the current session, if available.
    fn get_identity(&self) -> Result<Identity, GetIdentityError>;
}

impl IdentityExt for HttpRequest {
    fn get_identity(&self) -> Result<Identity, GetIdentityError> {
        Identity::extract(&self.extensions())
    }
}

impl IdentityExt for ServiceRequest {
    fn get_identity(&self) -> Result<Identity, GetIdentityError> {
        Identity::extract(&self.extensions())
    }
}

impl<'a> IdentityExt for GuardContext<'a> {
    fn get_identity(&self) -> Result<Identity, GetIdentityError> {
        Identity::extract(&self.req_data())
    }
}
