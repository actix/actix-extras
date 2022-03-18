use actix_session::UserSession;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{cookie::Cookie, http::header::COOKIE, web, Error, HttpResponse};
use futures::{
    future::{ok, Ready},
    Future,
};

use std::task::{Context, Poll};
use std::{cell::RefCell, pin::Pin, rc::Rc};

use crate::Limiter;

pub struct RateLimiter;

impl<S, B> Transform<S> for RateLimiter
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimiterMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimiterMiddleware {
            service: Rc::new(RefCell::new(service)),
        })
    }
}

pub struct RateLimiterMiddleware<S> {
    service: Rc<RefCell<S>>, // TODO: fix RefCell
}

type FutureType<R, E> = dyn Future<Output = Result<R, E>>;

impl<S, B> Service for RateLimiterMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<FutureType<Self::Response, Self::Error>>>;

    fn poll_ready(&mut self, context: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(context)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        // A mis-configuration of the Actix App will result in a **runtime** failure, so the expect
        // method description is important context for the developer.
        let limiter = req
            .app_data::<Limiter>()
            .expect("web::Data<Limiter> should be set in app data for RateLimiter middleware");

        let forbidden = HttpResponse::Forbidden().finish().into_body();
        let (key, fallback) = key(&req, limiter.clone());

        let mut service = self.service.clone();
        let key = match key {
            Some(key) => key,
            None => match fallback {
                Some(key) => key,
                None => {
                    return Box::pin(async move { service.call(req).await });
                }
            },
        };

        let mut service = self.service.clone();
        Box::pin(async move {
            let status = limiter.count(key.to_string()).await;
            if status.is_err() {
                warn!("403. Rate limit exceed error for {}", key);
                Ok(req.into_response(forbidden))
            } else {
                service.call(req).await
            }
        })
    }
}

fn key(req: &ServiceRequest, limiter: web::Data<Limiter>) -> (Option<String>, Option<String>) {
    let session = req.get_session();
    let result: Option<String> = session.get(&limiter.session_key).unwrap_or_else(|_| None);
    let cookies = req.headers().get_all(COOKIE);
    let cookie = cookies
        .map(|i| i.to_str().ok())
        .flatten()
        .find(|i| i.contains(&limiter.cookie_name));

    let fallback = match cookie {
        Some(value) => Cookie::parse(value).ok().map(|i| i.to_string()),
        None => None,
    };

    (result, fallback)
}
