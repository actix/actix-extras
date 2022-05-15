use actix_session::Session;
use actix_utils::future::{ready, Ready};
use actix_web::cookie::time::OffsetDateTime;
use actix_web::dev::Extensions;
use actix_web::http::StatusCode;
use actix_web::{dev::Payload, Error, FromRequest, HttpRequest};
use actix_web::{HttpMessage, HttpResponse};
use anyhow::{anyhow, Context};

use crate::configuration::LogoutBehaviour;

/// The extractor type to obtain your identity from a request.
///
/// ```
/// use actix_web::{get, post, Responder, HttpRequest, HttpMessage, HttpResponse};
/// use actix_identity::Identity;
///
/// #[get("/")]
/// async fn index(user: Option<Identity>) -> impl Responder {
///     if let Some(user) = user {
///         format!("Welcome! {}", user.id().unwrap())
///     } else {
///         "Welcome Anonymous!".to_owned()
///     }
/// }
///
/// #[post("/login")]
/// async fn login(request: HttpRequest) -> impl Responder {
///     Identity::login(&request.extensions(), "User1".into());
///     HttpResponse::Ok()
/// }
///
/// #[post("/logout")]
/// async fn logout(user: Identity) -> impl Responder {
///     user.logout();
///     HttpResponse::Ok()
/// }
/// ```
pub struct Identity(IdentityInner);

#[derive(Clone)]
pub(crate) struct IdentityInner {
    pub(crate) session: Session,
    pub(crate) logout_behaviour: LogoutBehaviour,
    pub(crate) is_login_deadline_enabled: bool,
}

impl IdentityInner {
    fn extract(e: &Extensions) -> Self {
        e.get::<Self>()
            .expect(
                "No `IdentityInner` instance was found in the extensions \
                attached to the incoming request. \
                This usually means that `IdentityMiddleware` has not been registered as an \
                application middleware via `App::wrap`. \
                `Identity` cannot be used unless the identity machine is properly mounted: register \
                `IdentityMiddleware` as a middleware for your application to fix this panic. \
                If the problem persists, please file an issue on GitHub.",
            )
            .to_owned()
    }

    /// Retrieve the user id attached to the current session.
    fn get_identity(&self) -> Result<String, anyhow::Error> {
        self.session
            .get::<String>(ID_KEY)
            .context("Failed to deserialize the user identifier attached to the current session")?
            .ok_or_else(|| {
                anyhow!("There is no identity information attached to the current session")
            })
    }
}

pub(crate) const ID_KEY: &str = "actix_identity.user_id";
// pub(crate) const LAST_VISIT_KEY: &str = "last_visited_at";
pub(crate) const LOGIN_TIMESTAMP_KEY: &str = "actix_identity.logged_in_at";

impl Identity {
    /// Return the user id associated to the current session.  
    ///
    /// ```
    /// use actix_web::{get, Responder};
    /// use actix_identity::Identity;
    ///
    /// #[get("/")]
    /// async fn index(user: Option<Identity>) -> impl Responder {
    ///     if let Some(user) = user {
    ///         format!("Welcome! {}", user.id().unwrap())
    ///     } else {
    ///         "Welcome Anonymous!".to_owned()
    ///     }
    /// }
    /// ```
    pub fn id(&self) -> Result<String, anyhow::Error> {
        self.0.session.get(ID_KEY)?.ok_or_else(|| {
            anyhow!("Bug: the identity information attached to the current session has disappeared")
        })
    }

    /// Attach a valid user identity to the current session.  
    /// This method should be called after you have successfully authenticated the user. After
    /// `login` has been called, the user will be able to access all routes that require a
    /// valid [`Identity`].
    ///
    /// ```
    /// use actix_web::{post, Responder, HttpRequest, HttpMessage, HttpResponse};
    /// use actix_identity::Identity;
    ///
    /// #[post("/login")]
    /// async fn login(request: HttpRequest) -> impl Responder {
    ///     Identity::login(&request.extensions(), "User1".into());
    ///     HttpResponse::Ok()
    /// }
    /// ```
    pub fn login(e: &Extensions, id: String) -> Result<Self, anyhow::Error> {
        let inner = IdentityInner::extract(e);
        inner.session.insert(ID_KEY, id)?;
        inner.session.insert(
            LOGIN_TIMESTAMP_KEY,
            OffsetDateTime::now_utc().unix_timestamp(),
        )?;
        inner.session.renew();
        Ok(Self(inner))
    }

    /// Remove the user identity from the current session.  
    /// After `logout` has been called, the user will no longer be able to access routes that
    /// require a valid [`Identity`].
    ///
    /// The behaviour on logout is determined by
    /// [`IdentityMiddlewareBuilder::logout_behaviour`](crate::configuration::IdentityMiddlewareBuilder::logout_behaviour).
    ///
    /// ```
    /// use actix_web::{post, Responder, HttpResponse};
    /// use actix_identity::Identity;
    ///
    /// #[post("/logout")]
    /// async fn logout(user: Identity) -> impl Responder {
    ///     user.logout();
    ///     HttpResponse::Ok()
    /// }
    /// ```
    pub fn logout(self) {
        match self.0.logout_behaviour {
            LogoutBehaviour::PurgeSession => {
                self.0.session.purge();
            }
            LogoutBehaviour::DeleteIdentityKeys => {
                self.0.session.remove(ID_KEY);
                if self.0.is_login_deadline_enabled {
                    self.0.session.remove(LOGIN_TIMESTAMP_KEY);
                }
            }
        }
    }

    pub(crate) fn extract(e: &Extensions) -> Result<Self, anyhow::Error> {
        let inner = IdentityInner::extract(e);
        inner.get_identity()?;
        Ok(Self(inner))
    }

    pub(crate) fn logged_at(&self) -> Result<Option<OffsetDateTime>, anyhow::Error> {
        self.0
            .session
            .get(LOGIN_TIMESTAMP_KEY)?
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(anyhow::Error::from)
    }
}

/// Extractor implementation for [`Identity`].
///
/// ```
/// use actix_web::{get, Responder};
/// use actix_identity::Identity;
///
/// #[get("/")]
/// async fn index(user: Option<Identity>) -> impl Responder {
///     if let Some(user) = user {
///         format!("Welcome! {}", user.id().unwrap())
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
        let r = Identity::extract(&req.extensions()).map_err(|e| {
            let e = actix_web::error::InternalError::from_response(
                e,
                HttpResponse::new(StatusCode::UNAUTHORIZED),
            );
            actix_web::Error::from(e)
        });
        ready(r)
    }
}
