use std::{collections::HashMap, future::Future, pin::Pin, rc::Rc};

use actix_utils::future::{ok, Ready};
use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    web, Error, HttpResponse,
};

use crate::{Error as LimitationError, Limiter};

/// Rate limit middleware.
///
/// Use the `scope` variable to define multiple limiter
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct RateLimiter {
    /// Used to define multiple limiter, with different configurations
    ///
    /// WARNING: When used (not None) the middleware will expect a `HashMap<Limiter>` in the actix-web `app_data`
    pub scope: Option<&'static str>,
}

impl RateLimiter {
    /// Construct the rate limiter with a scope
    pub fn scoped(scope: &'static str) -> Self {
        RateLimiter { scope: Some(scope) }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimiterMiddleware {
            service: Rc::new(service),
            scope: self.scope,
        })
    }
}

/// Rate limit middleware service.
#[derive(Debug)]
pub struct RateLimiterMiddleware<S> {
    service: Rc<S>,
    scope: Option<&'static str>,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // A mis-configuration of the Actix App will result in a **runtime** failure, so the expect
        // method description is important context for the developer.
        let limiter = if let Some(scope) = self.scope {
            let limiters = req.app_data::<web::Data<HashMap<&str, Limiter>>>().expect(
                "web::Data<HashMap<Limiter>> should be set in app data for RateLimiter middleware",
            );
            limiters
                .get(scope)
                .unwrap_or_else(|| panic!("Unable to find defined limiter with scope: {}", scope))
                .clone()
        } else {
            let limiter = req
                .app_data::<web::Data<Limiter>>()
                .expect("web::Data<Limiter> should be set in app data for RateLimiter middleware");
            // Deref to get the Limiter
            (***limiter).clone()
        };

        let key = (limiter.get_key_fn)(&req);
        let service = Rc::clone(&self.service);

        let key = match key {
            Some(key) => key,
            None => {
                return Box::pin(async move {
                    service
                        .call(req)
                        .await
                        .map(ServiceResponse::map_into_left_body)
                });
            }
        };

        Box::pin(async move {
            let status = limiter.count(key.to_string()).await;

            if let Err(err) = status {
                match err {
                    LimitationError::LimitExceeded(_) => {
                        log::warn!("Rate limit exceed error for {}", key);

                        Ok(req.into_response(
                            HttpResponse::new(StatusCode::TOO_MANY_REQUESTS).map_into_right_body(),
                        ))
                    }
                    LimitationError::Client(e) => {
                        log::error!("Client request failed, redis error: {}", e);

                        Ok(req.into_response(
                            HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
                                .map_into_right_body(),
                        ))
                    }
                    _ => {
                        log::error!("Count failed: {}", err);

                        Ok(req.into_response(
                            HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
                                .map_into_right_body(),
                        ))
                    }
                }
            } else {
                service
                    .call(req)
                    .await
                    .map(ServiceResponse::map_into_left_body)
            }
        })
    }
}
