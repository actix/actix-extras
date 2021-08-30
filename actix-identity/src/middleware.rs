use std::{error::Error as StdError, rc::Rc};

use actix_web::{
    body::{AnyBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, Result,
};
use futures_util::future::{ready, FutureExt as _, LocalBoxFuture, Ready, TryFutureExt as _};

use crate::{identity::IdentityItem, IdentityPolicy};

/// Request identity middleware
///
/// ```
/// use actix_web::App;
/// use actix_identity::{CookieIdentityPolicy, IdentityService};
///
/// // create cookie identity backend
/// let policy = CookieIdentityPolicy::new(&[0; 32])
///            .name("auth-cookie")
///            .secure(false);
///
/// let app = App::new()
///     // wrap policy into identity middleware
///     .wrap(IdentityService::new(policy));
/// ```
pub struct IdentityService<T> {
    backend: Rc<T>,
}

impl<T> IdentityService<T> {
    /// Create new identity service with specified backend.
    pub fn new(backend: T) -> Self {
        IdentityService {
            backend: Rc::new(backend),
        }
    }
}

impl<S, T, B> Transform<S, ServiceRequest> for IdentityService<T>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    T: IdentityPolicy,
    B: MessageBody + 'static,
    B::Error: StdError,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = IdentityServiceMiddleware<S, T>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(IdentityServiceMiddleware {
            backend: self.backend.clone(),
            service: Rc::new(service),
        }))
    }
}

pub struct IdentityServiceMiddleware<S, T> {
    pub(crate) service: Rc<S>,
    pub(crate) backend: Rc<T>,
}

impl<S, T> Clone for IdentityServiceMiddleware<S, T> {
    fn clone(&self) -> Self {
        Self {
            backend: Rc::clone(&self.backend),
            service: Rc::clone(&self.service),
        }
    }
}

impl<S, T, B> Service<ServiceRequest> for IdentityServiceMiddleware<S, T>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    T: IdentityPolicy,
    B: MessageBody + 'static,
    B::Error: StdError,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_service::forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let srv = Rc::clone(&self.service);
        let backend = Rc::clone(&self.backend);
        let fut = self.backend.from_request(&mut req);

        async move {
            match fut.await {
                Ok(id) => {
                    req.extensions_mut()
                        .insert(IdentityItem { id, changed: false });

                    let mut res = srv.call(req).await?;
                    let id = res.request().extensions_mut().remove::<IdentityItem>();

                    if let Some(id) = id {
                        match backend.to_response(id.id, id.changed, &mut res).await {
                            Ok(_) => Ok(res.map_body(|_, body| AnyBody::from_message(body))),
                            Err(e) => Ok(res.error_response(e)),
                        }
                    } else {
                        Ok(res.map_body(|_, body| AnyBody::from_message(body)))
                    }
                }
                Err(err) => Ok(req.error_response(err)),
            }
        }
        .map_ok(|res| res.map_body(|_, body| AnyBody::from_message(body)))
        .boxed_local()
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use actix_service::into_service;
    use actix_web::{dev, error, test, Error, Result};

    use super::*;

    #[actix_rt::test]
    async fn test_borrowed_mut_error() {
        use futures_util::future::{lazy, ok, Ready};

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

        let srv = crate::middleware::IdentityServiceMiddleware {
            backend: Rc::new(Ident),
            service: Rc::new(into_service(|_: dev::ServiceRequest| async move {
                actix_rt::time::sleep(Duration::from_secs(100)).await;
                Err::<dev::ServiceResponse, _>(error::ErrorBadRequest("error"))
            })),
        };

        let srv2 = srv.clone();
        let req = test::TestRequest::default().to_srv_request();

        actix_rt::spawn(async move {
            let _ = srv2.call(req).await;
        });

        actix_rt::time::sleep(Duration::from_millis(50)).await;

        let _ = lazy(|cx| srv.poll_ready(cx)).await;
    }
}
