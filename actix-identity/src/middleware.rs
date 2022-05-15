use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use actix_session::SessionExt;
use actix_utils::future::{ready, Ready};
use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage as _, Result,
    cookie::time::format_description::well_known::Rfc3339
};
use time::OffsetDateTime;

use crate::configuration::{Configuration, IdentityMiddlewareBuilder};
use crate::identity::IdentityInner;
use crate::Identity;

/// Request identity middleware
///
/// ```no_run
/// use actix_web::{HttpServer, cookie::Key, App};
/// use actix_session::storage::RedisSessionStore;
/// use actix_identity::{Identity, IdentityMiddleware};
/// use actix_session::{Session, SessionMiddleware};
///
/// #[actix_web::main]
/// async fn main() {
///     let secret_key = Key::generate();
///     let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379").await.unwrap();
///     HttpServer::new(move || {
///        App::new()
///            // Install the identity framework.
///            .wrap(IdentityMiddleware::default())
///            // The identity system is built on top of sessions.
///            // You must install the session middleware to leverage `actix-identity`.
///            .wrap(SessionMiddleware::new(redis_store.clone(), secret_key.clone()))
///     })
/// # ;
/// }
/// ```
#[derive(Default, Clone)]
pub struct IdentityMiddleware {
    configuration: Rc<Configuration>,
}

impl IdentityMiddleware {
    pub(crate) fn new(configuration: Configuration) -> Self {
        Self {
            configuration: Rc::new(configuration),
        }
    }

    /// A fluent API to configure [`IdentityMiddleware`].
    pub fn builder() -> IdentityMiddlewareBuilder {
        IdentityMiddlewareBuilder::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for IdentityMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = InnerIdentityMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(InnerIdentityMiddleware {
            service: Rc::new(service),
            configuration: Rc::clone(&self.configuration),
        }))
    }
}

#[doc(hidden)]
pub struct InnerIdentityMiddleware<S> {
    service: Rc<S>,
    configuration: Rc<Configuration>,
}

impl<S> Clone for InnerIdentityMiddleware<S> {
    fn clone(&self) -> Self {
        Self {
            service: Rc::clone(&self.service),
            configuration: Rc::clone(&self.configuration),
        }
    }
}

impl<S, B> Service<ServiceRequest> for InnerIdentityMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_service::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let srv = Rc::clone(&self.service);
        let configuration = Rc::clone(&self.configuration);
        Box::pin(async move {
            let identity_inner = IdentityInner {
                session: req.get_session(),
                logout_behaviour: configuration.on_logout.clone(),
                is_login_deadline_enabled: configuration.login_deadline.is_some(),
            };
            req.extensions_mut().insert(identity_inner);
            if let Some(login_deadline) = configuration.login_deadline {
                match Identity::extract(&req.extensions()) {
                    Ok(identity) => {
                        match identity.logged_at() {
                            Ok(None) => {
                                tracing::info!("Login deadline is enabled, but there is no login timestamp in the session state attached to the incoming request. Logging the user out.");
                                identity.logout();
                            }
                            Err(e) => {
                                tracing::info!(
                                    error.display = %e,
                                    error.debug = ?e,
                                    "Login deadline is enabled and we failed to extract the login timestamp from the session state attached to the incoming request. Logging the user out."
                                );
                                identity.logout();
                            }
                            Ok(Some(logged_in_at)) => {
                                let elapsed = OffsetDateTime::now_utc() - logged_in_at;
                                if elapsed > login_deadline {
                                    tracing::info!(
                                        user.logged_in_at = %logged_in_at.format(&Rfc3339).unwrap_or_else(|_| "".into()),
                                        identity.login_deadline_seconds = login_deadline.as_secs(),
                                        identity.elapsed_since_login_seconds = elapsed.whole_seconds(),
                                        "Login deadline is enabled and too much time has passed since the user logged in. Logging the user out."
                                    );
                                    identity.logout();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            error.display = %e,
                            error.debug = ?e,
                            "Failed to extract an `Identity` from the incoming request."
                        );
                    }
                }
            }
            srv.call(req).await
        })
    }
}
