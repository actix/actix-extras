//! Type-safe authentication information extractors

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use actix_web::dev::ServiceRequest;
use actix_web::Error;
use futures_core::ready;
use pin_project_lite::pin_project;

pub mod basic;
pub mod bearer;
mod config;
mod errors;

pub use self::config::AuthExtractorConfig;
pub use self::errors::AuthenticationError;

/// Trait implemented by types that can extract
/// HTTP authentication scheme credentials from the request.
///
/// It is very similar to actix' `FromRequest` trait,
/// except it operates with a `ServiceRequest` struct instead,
/// therefore it can be used in the middlewares.
///
/// You will not need it unless you want to implement your own
/// authentication scheme.
pub trait AuthExtractor: Sized {
    /// The associated error which can be returned.
    type Error: Into<Error>;

    /// Future that resolves into extracted credentials type.
    type Future: Future<Output = Result<Self, Self::Error>>;

    /// Parse the authentication credentials from the actix' `ServiceRequest`.
    fn from_service_request(req: &ServiceRequest) -> Self::Future;
}

impl<T: AuthExtractor> AuthExtractor for Option<T> {
    type Error = T::Error;

    type Future = AuthExtractorOptFut<T::Future>;

    fn from_service_request(req: &ServiceRequest) -> Self::Future {
        let fut = T::from_service_request(req);
        AuthExtractorOptFut { fut }
    }
}

pin_project! {
    #[doc(hidden)]
    pub struct AuthExtractorOptFut<F> {
        #[pin]
        fut: F
    }
}

impl<F, T, E> Future for AuthExtractorOptFut<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<Option<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().fut.poll(cx));
        Poll::Ready(Ok(res.ok()))
    }
}

impl<T: AuthExtractor> AuthExtractor for Result<T, T::Error> {
    type Error = T::Error;

    type Future = AuthExtractorResFut<T::Future>;

    fn from_service_request(req: &ServiceRequest) -> Self::Future {
        AuthExtractorResFut {
            fut: T::from_service_request(req),
        }
    }
}

pin_project! {
    #[doc(hidden)]
    pub struct AuthExtractorResFut<F> {
        #[pin]
        fut: F
    }
}

impl<F, T, E> Future for AuthExtractorResFut<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<F::Output, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().fut.poll(cx));
        Poll::Ready(Ok(res))
    }
}
