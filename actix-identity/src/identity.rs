use actix_session::Session;
use actix_utils::future::{ready, Ready};
use actix_web::{
    cookie::time::OffsetDateTime,
    dev::{Extensions, Payload},
    http::StatusCode,
    Error, FromRequest, HttpMessage, HttpRequest, HttpResponse,
};
use anyhow::{anyhow, Context};

use crate::config::LogoutBehaviour;

/// A verified user identity. It can be used as a request extractor.
///
/// The lifecycle of a user identity is tied to the lifecycle of the underlying session. If the
/// session is destroyed (e.g. the session expired), the user identity will be forgotten, de-facto
/// forcing a user log out.
///
/// # Examples
/// ```
/// use actix_web::{
///     get, post, Responder, HttpRequest, HttpMessage, HttpResponse
/// };
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
///
/// # Extractor Behaviour
/// What happens if you try to extract an `Identity` out of a request that does not have a valid
/// identity attached? The API will return a `401 UNAUTHORIZED` to the caller.
///
/// If you want to customise this behaviour, consider extracting `Option<Identity>` or
/// `Result<Identity, actix_web::Error>` instead of a bare `Identity`: you will then be fully in
/// control of the error path.
///
/// ## Examples
/// ```
/// use actix_web::{http::header::LOCATION, get, HttpResponse, Responder};
/// use actix_identity::Identity;
///
/// #[get("/")]
/// async fn index(user: Option<Identity>) -> impl Responder {
///     if let Some(user) = user {
///         HttpResponse::Ok().finish()
///     } else {
///         // Redirect to login page if unauthenticated
///         HttpResponse::TemporaryRedirect()
///             .insert_header((LOCATION, "/login"))
///             .finish()
///     }
/// }
/// ```
pub struct Identity(IdentityInner);

#[derive(Clone)]
pub(crate) struct IdentityInner {
    pub(crate) session: Session,
    pub(crate) logout_behaviour: LogoutBehaviour,
    pub(crate) is_login_deadline_enabled: bool,
    pub(crate) is_visit_deadline_enabled: bool,
}

impl IdentityInner {
    fn extract(ext: &Extensions) -> Self {
        ext.get::<Self>()
            .expect(
                "No `IdentityInner` instance was found in the extensions attached to the \
                incoming request. This usually means that `IdentityMiddleware` has not been \
                registered as an application middleware via `App::wrap`. `Identity` cannot be used \
                unless the identity machine is properly mounted: register `IdentityMiddleware` as \
                a middleware for your application to fix this panic. If the problem persists, \
                please file an issue on GitHub.",
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
pub(crate) const LAST_VISIT_UNIX_TIMESTAMP_KEY: &str = "actix_identity.last_visited_at";
pub(crate) const LOGIN_UNIX_TIMESTAMP_KEY: &str = "actix_identity.logged_in_at";

impl Identity {
    /// Return the user id associated to the current session.
    ///
    /// # Examples
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
    ///
    /// This method should be called after you have successfully authenticated the user. After
    /// `login` has been called, the user will be able to access all routes that require a valid
    /// [`Identity`].
    ///
    /// # Examples
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
    pub fn login(ext: &Extensions, id: String) -> Result<Self, anyhow::Error> {
        let inner = IdentityInner::extract(ext);
        inner.session.insert(ID_KEY, id)?;
        inner.session.insert(
            LOGIN_UNIX_TIMESTAMP_KEY,
            OffsetDateTime::now_utc().unix_timestamp(),
        )?;
        inner.session.renew();
        Ok(Self(inner))
    }

    /// Remove the user identity from the current session.
    ///
    /// After `logout` has been called, the user will no longer be able to access routes that
    /// require a valid [`Identity`].
    ///
    /// The behaviour on logout is determined by [`IdentityMiddlewareBuilder::logout_behaviour`].
    ///
    /// # Examples
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
    ///
    /// [`IdentityMiddlewareBuilder::logout_behaviour`]: crate::config::IdentityMiddlewareBuilder::logout_behaviour
    pub fn logout(self) {
        match self.0.logout_behaviour {
            LogoutBehaviour::PurgeSession => {
                self.0.session.purge();
            }
            LogoutBehaviour::DeleteIdentityKeys => {
                self.0.session.remove(ID_KEY);
                if self.0.is_login_deadline_enabled {
                    self.0.session.remove(LOGIN_UNIX_TIMESTAMP_KEY);
                }
                if self.0.is_visit_deadline_enabled {
                    self.0.session.remove(LAST_VISIT_UNIX_TIMESTAMP_KEY);
                }
            }
        }
    }

    pub(crate) fn extract(ext: &Extensions) -> Result<Self, anyhow::Error> {
        let inner = IdentityInner::extract(ext);
        inner.get_identity()?;
        Ok(Self(inner))
    }

    pub(crate) fn logged_at(&self) -> Result<Option<OffsetDateTime>, anyhow::Error> {
        self.0
            .session
            .get(LOGIN_UNIX_TIMESTAMP_KEY)?
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(anyhow::Error::from)
    }

    pub(crate) fn last_visited_at(&self) -> Result<Option<OffsetDateTime>, anyhow::Error> {
        self.0
            .session
            .get(LAST_VISIT_UNIX_TIMESTAMP_KEY)?
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(anyhow::Error::from)
    }
}

/// Extractor implementation for [`Identity`].
///
/// # Examples
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
        ready(Identity::extract(&req.extensions()).map_err(|err| {
            let res = actix_web::error::InternalError::from_response(
                err,
                HttpResponse::new(StatusCode::UNAUTHORIZED),
            );

            actix_web::Error::from(res)
        }))
    }
}
