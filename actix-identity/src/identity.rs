use crate::configuration::LogoutBehaviour;
use actix_session::Session;
use actix_utils::future::{ready, Ready};
use actix_web::dev::Extensions;
use actix_web::HttpMessage;
use actix_web::{dev::Payload, Error, FromRequest, HttpRequest};

/// The extractor type to obtain your identity from a request.
///
/// ```
/// use actix_web::*;
/// use actix_identity::Identity;
/// use actix_session::Session;
///
/// #[get("/")]
/// async fn index(id: Identity) -> impl Responder {
///     // access request identity
///     if let Some(id) = id.id() {
///         format!("Welcome! {}", id)
///     } else {
///         "Welcome Anonymous!".to_owned()
///     }
/// }
///
/// #[post("/login")]
/// async fn login(session: Session) -> impl Responder {
///     Identity::login("User1".to_owned(), session);
///     HttpResponse::Ok()
/// }
///
/// #[post("/logout")]
/// async fn logout(id: Identity) -> impl Responder {
///     id.logout();
///     HttpResponse::Ok()
/// }
/// ```
#[derive(Clone)]
pub struct Identity(IdentityInner);

#[derive(Clone)]
pub(crate) struct IdentityInner {
    pub(crate) session: Session,
    pub(crate) logout_behaviour: LogoutBehaviour,
}

pub(crate) const ID_KEY: &str = "user_id";

impl Identity {
    /// Return the claimed identity of the user associated request or `None` if no identity can be
    /// found associated with the request.
    pub fn id(&self) -> Option<String> {
        self.0.session.get(ID_KEY).ok().flatten()
    }

    /// Attach a valid user identity to the current session.  
    /// This method should be called after you have successfully authenticated the user. After
    /// `login` has been called, the user will be able to access all routes that require a
    /// valid [`Identity`].
    // TODO: what happens if the user is already logged in?
    pub fn login(extensions: &Extensions, id: String) -> Result<Self, anyhow::Error> {
        let identity = Self::extract(&extensions);
        identity.0.session.insert(ID_KEY, id)?;
        Ok(identity)
    }

    /// Remove the user identity from the current session.  
    /// After `logout` has been called, the user will no longer be able to access routes that
    /// require a valid [`Identity`].
    ///
    /// The behaviour on logout is determined by
    /// [`IdentityMiddlewareBuilder::logout_behaviour`](crate::configuration::IdentityMiddlewareBuilder::logout_behaviour).
    pub fn logout(self) {
        match self.0.logout_behaviour {
            LogoutBehaviour::PurgeSession => {
                self.0.session.purge();
            }
            LogoutBehaviour::DeleteIdentityKeys => {
                self.0.session.remove(ID_KEY);
            }
        }
    }

    pub(crate) fn extract(e: &Extensions) -> Self {
        // TODO: review unwrap
        let inner = e.get::<IdentityInner>().unwrap().to_owned();
        Self(inner)
    }
}

/// Extractor implementation for [`Identity`].
///
/// ```
/// # use actix_web::*;
/// use actix_identity::Identity;
///
/// #[get("/")]
/// async fn index(id: Identity) -> impl Responder {
///     // access request identity
///     if let Some(id) = id.id() {
///         format!("Welcome! {}", id)
///     } else {
///         "Welcome Anonymous!".to_owned()
///     }
/// }
/// ```
impl FromRequest for Identity {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(Ok(Identity::extract(&req.extensions())))
    }
}
