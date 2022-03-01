use actix_utils::future::{ready, Ready};
use actix_web::{
    dev::{Extensions, Payload},
    Error, FromRequest, HttpMessage as _, HttpRequest,
};

pub(crate) struct IdentityItem {
    pub(crate) id: Option<String>,
    pub(crate) changed: bool,
}

/// The extractor type to obtain your identity from a request.
///
/// ```
/// use actix_web::*;
/// use actix_identity::Identity;
///
/// #[get("/")]
/// async fn index(id: Identity) -> impl Responder {
///     // access request identity
///     if let Some(id) = id.identity() {
///         format!("Welcome! {}", id)
///     } else {
///         "Welcome Anonymous!".to_owned()
///     }
/// }
///
/// #[post("/login")]
/// async fn login(id: Identity) -> impl Responder {
///     // remember identity
///     id.remember("User1".to_owned());
///
///     HttpResponse::Ok()
/// }
///
/// #[post("/logout")]
/// async fn logout(id: Identity) -> impl Responder {
///     // remove identity
///     id.forget();
///
///     HttpResponse::Ok()
/// }
/// ```
#[derive(Clone)]
pub struct Identity(HttpRequest);

impl Identity {
    /// Return the claimed identity of the user associated request or `None` if no identity can be
    /// found associated with the request.
    pub fn identity(&self) -> Option<String> {
        Identity::get_identity(&self.0.extensions())
    }

    /// Remember identity.
    pub fn remember(&self, identity: String) {
        if let Some(id) = self.0.extensions_mut().get_mut::<IdentityItem>() {
            id.id = Some(identity);
            id.changed = true;
        }
    }

    /// This method is used to 'forget' the current identity on subsequent requests.
    pub fn forget(&self) {
        if let Some(id) = self.0.extensions_mut().get_mut::<IdentityItem>() {
            id.id = None;
            id.changed = true;
        }
    }

    pub(crate) fn get_identity(extensions: &Extensions) -> Option<String> {
        let id = extensions.get::<IdentityItem>()?;
        id.id.clone()
    }
}

/// Extractor implementation for Identity type.
///
/// ```
/// # use actix_web::*;
/// use actix_identity::Identity;
///
/// #[get("/")]
/// async fn index(id: Identity) -> impl Responder {
///     // access request identity
///     if let Some(id) = id.identity() {
///         format!("Welcome! {}", id)
///     } else {
///         "Welcome Anonymous!".to_owned()
///     }
/// }
/// ```
impl FromRequest for Identity {
    type Error = Error;
    type Future = Ready<Result<Identity, Error>>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(Ok(Identity(req.clone())))
    }
}
