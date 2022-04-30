use crate::configuration::{Configuration, IdentityMiddlewareBuilder};
use crate::identity::IdentityInner;
use actix_session::SessionExt;
use actix_utils::future::{ready, Ready};
use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage as _, Result,
};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// Request identity middleware
///
/// ```
/// use actix_web::App;
/// use actix_identity::{CookieIdentityPolicy, IdentityMiddleware};
///
/// // create cookie identity backend
/// let policy = CookieIdentityPolicy::new(&[0; 32])
///            .name("auth-cookie")
///            .secure(false);
///
/// let app = App::new()
///     // wrap policy into identity middleware
///     .wrap(IdentityMiddleware::new(policy));
/// ```
#[derive(Default)]
pub struct IdentityMiddleware {
    configuration: Rc<Configuration>,
}

impl IdentityMiddleware {
    pub(crate) fn new(configuration: Configuration) -> Self {
        Self {
            configuration: Rc::new(configuration),
        }
    }

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
            };
            req.extensions_mut().insert(identity_inner);
            srv.call(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use actix_service::into_service;
    use actix_web::{dev, error, test, Error, Result};

    use super::*;

    #[actix_web::test]
    async fn test_borrowed_mut_error() {
        use actix_utils::future::{ok, Ready};
        use futures_util::future::lazy;

        struct Ident;
        impl IdentityPolicy for Ident {
            type Future = Ready<Result<Option<String>, Error>>;
            type ResponseFuture = Ready<Result<(), Error>>;

            fn from_request(&self, _: &mut dev::ServiceRequest) -> Self::Future {
                ok(Some("test".to_string()))
            }

            fn to_response<B>(
                &self,
                _: Option<String>,
                _: bool,
                _: &mut dev::ServiceResponse<B>,
            ) -> Self::ResponseFuture {
                ok(())
            }
        }

        let srv = crate::middleware::InnerIdentityMiddleware {
            backend: Rc::new(Ident),
            service: Rc::new(into_service(|_: dev::ServiceRequest| async move {
                actix_web::rt::time::sleep(Duration::from_secs(100)).await;
                Err::<dev::ServiceResponse, _>(error::ErrorBadRequest("error"))
            })),
        };

        let srv2 = srv.clone();
        let req = test::TestRequest::default().to_srv_request();

        actix_web::rt::spawn(async move {
            let _ = srv2.call(req).await;
        });

        actix_web::rt::time::sleep(Duration::from_millis(50)).await;

        let _ = lazy(|cx| srv.poll_ready(cx)).await;
    }
}
