//! HTTP Authentication middleware.

use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures_util::future::{self, FutureExt, LocalBoxFuture, TryFutureExt};
use futures_util::task::{Context, Poll};

use crate::extractors::{basic, bearer, AuthExtractor};

/// Middleware for checking HTTP authentication.
///
/// If there is no `Authorization` header in the request,
/// this middleware returns an error immediately,
/// without calling the `F` callback.
///
/// Otherwise, it will pass both the request and
/// the parsed credentials into it.
/// In case of successful validation `F` callback
/// is required to return the `ServiceRequest` back.
#[derive(Debug, Clone)]
pub struct HttpAuthentication<T, F>
where
    T: AuthExtractor,
{
    process_fn: Arc<F>,
    _extractor: PhantomData<T>,
}

impl<T, F, O> HttpAuthentication<T, F>
where
    T: AuthExtractor,
    F: Fn(ServiceRequest, T) -> O,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware
    /// with the provided auth extractor `T` and
    /// validation callback `F`.
    pub fn with_fn(process_fn: F) -> HttpAuthentication<T, F> {
        HttpAuthentication {
            process_fn: Arc::new(process_fn),
            _extractor: PhantomData,
        }
    }
}

impl<F, O> HttpAuthentication<basic::BasicAuth, F>
where
    F: Fn(ServiceRequest, basic::BasicAuth) -> O,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Basic"
    /// authentication scheme.
    ///
    /// ## Example
    ///
    /// ```
    /// # use actix_web::Error;
    /// # use actix_web::dev::ServiceRequest;
    /// # use actix_web_httpauth::middleware::HttpAuthentication;
    /// # use actix_web_httpauth::extractors::basic::BasicAuth;
    /// // In this example validator returns immediately,
    /// // but since it is required to return anything
    /// // that implements `IntoFuture` trait,
    /// // it can be extended to query database
    /// // or to do something else in a async manner.
    /// async fn validator(
    ///     req: ServiceRequest,
    ///     credentials: BasicAuth,
    /// ) -> Result<ServiceRequest, Error> {
    ///     // All users are great and more than welcome!
    ///     Ok(req)
    /// }
    ///
    /// let middleware = HttpAuthentication::basic(validator);
    /// ```
    pub fn basic(process_fn: F) -> Self {
        Self::with_fn(process_fn)
    }
}

impl<F, O> HttpAuthentication<bearer::BearerAuth, F>
where
    F: Fn(ServiceRequest, bearer::BearerAuth) -> O,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Bearer"
    /// authentication scheme.
    ///
    /// ## Example
    ///
    /// ```
    /// # use actix_web::Error;
    /// # use actix_web::dev::ServiceRequest;
    /// # use actix_web_httpauth::middleware::HttpAuthentication;
    /// # use actix_web_httpauth::extractors::bearer::{Config, BearerAuth};
    /// # use actix_web_httpauth::extractors::{AuthenticationError, AuthExtractorConfig};
    /// async fn validator(req: ServiceRequest, credentials: BearerAuth) -> Result<ServiceRequest, Error> {
    ///     if credentials.token() == "mF_9.B5f-4.1JqM" {
    ///         Ok(req)
    ///     } else {
    ///         let config = req.app_data::<Config>()
    ///             .map(|data| data.clone())
    ///             .unwrap_or_else(Default::default)
    ///             .scope("urn:example:channel=HBO&urn:example:rating=G,PG-13");
    ///
    ///         Err(AuthenticationError::from(config).into())
    ///     }
    /// }
    ///
    /// let middleware = HttpAuthentication::bearer(validator);
    /// ```
    pub fn bearer(process_fn: F) -> Self {
        Self::with_fn(process_fn)
    }
}

impl<S, B, T, F, O> Transform<S> for HttpAuthentication<T, F>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + 'static,
    O: Future<Output = Result<ServiceRequest, Error>> + 'static,
    T: AuthExtractor + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthenticationMiddleware<S, F, T>;
    type InitError = ();
    type Future = future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(AuthenticationMiddleware {
            service: Rc::new(RefCell::new(service)),
            process_fn: self.process_fn.clone(),
            _extractor: PhantomData,
        })
    }
}

#[doc(hidden)]
pub struct AuthenticationMiddleware<S, F, T>
where
    T: AuthExtractor,
{
    service: Rc<RefCell<S>>,
    process_fn: Arc<F>,
    _extractor: PhantomData<T>,
}

impl<S, B, F, T, O> Service for AuthenticationMiddleware<S, F, T>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + 'static,
    O: Future<Output = Result<ServiceRequest, Error>> + 'static,
    T: AuthExtractor + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<ServiceResponse<B>, Error>>;

    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.borrow_mut().poll_ready(ctx)
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let process_fn = self.process_fn.clone();

        let service = Rc::clone(&self.service);

        async move {
            let (req, credentials) = Extract::<T>::new(req).await?;
            let req = process_fn(req, credentials).await?;
            // It is important that `borrow_mut()` and `.await` are on
            // separate lines, or else a panic occurs.
            let fut = service.borrow_mut().call(req);
            fut.await
        }
        .boxed_local()
    }
}

struct Extract<T> {
    req: Option<ServiceRequest>,
    f: Option<LocalBoxFuture<'static, Result<T, Error>>>,
    _extractor: PhantomData<fn() -> T>,
}

impl<T> Extract<T> {
    pub fn new(req: ServiceRequest) -> Self {
        Extract {
            req: Some(req),
            f: None,
            _extractor: PhantomData,
        }
    }
}

impl<T> Future for Extract<T>
where
    T: AuthExtractor,
    T::Future: 'static,
    T::Error: 'static,
{
    type Output = Result<(ServiceRequest, T), Error>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.f.is_none() {
            let req = self.req.as_ref().expect("Extract future was polled twice!");
            let f = T::from_service_request(req).map_err(Into::into);
            self.f = Some(f.boxed_local());
        }

        let f = self
            .f
            .as_mut()
            .expect("Extraction future should be initialized at this point");
        let credentials = futures_util::ready!(Future::poll(f.as_mut(), ctx))?;

        let req = self.req.take().expect("Extract future was polled twice!");
        Poll::Ready(Ok((req, credentials)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::bearer::BearerAuth;
    use actix_service::{into_service, Service};
    use actix_web::error;
    use actix_web::test::TestRequest;
    use futures_util::join;

    /// This is a test for https://github.com/actix/actix-extras/issues/10
    #[actix_rt::test]
    async fn test_middleware_panic() {
        let mut middleware = AuthenticationMiddleware {
            service: Rc::new(RefCell::new(into_service(
                |_: ServiceRequest| async move {
                    actix_rt::time::delay_for(std::time::Duration::from_secs(1)).await;
                    Err::<ServiceResponse, _>(error::ErrorBadRequest("error"))
                },
            ))),
            process_fn: Arc::new(|req, _: BearerAuth| async { Ok(req) }),
            _extractor: PhantomData,
        };

        let req = TestRequest::with_header("Authorization", "Bearer 1").to_srv_request();

        let f = middleware.call(req);

        let res = futures_util::future::lazy(|cx| middleware.poll_ready(cx));

        assert!(join!(f, res).0.is_err());
    }

    /// This is a test for https://github.com/actix/actix-extras/issues/10
    #[actix_rt::test]
    async fn test_middleware_panic_several_orders() {
        let mut middleware = AuthenticationMiddleware {
            service: Rc::new(RefCell::new(into_service(
                |_: ServiceRequest| async move {
                    actix_rt::time::delay_for(std::time::Duration::from_secs(1)).await;
                    Err::<ServiceResponse, _>(error::ErrorBadRequest("error"))
                },
            ))),
            process_fn: Arc::new(|req, _: BearerAuth| async { Ok(req) }),
            _extractor: PhantomData,
        };

        let req = TestRequest::with_header("Authorization", "Bearer 1").to_srv_request();

        let f1 = middleware.call(req);

        let req = TestRequest::with_header("Authorization", "Bearer 1").to_srv_request();

        let f2 = middleware.call(req);

        let req = TestRequest::with_header("Authorization", "Bearer 1").to_srv_request();

        let f3 = middleware.call(req);

        let res = futures_util::future::lazy(|cx| middleware.poll_ready(cx));

        let result = join!(f1, f2, f3, res);

        assert!(result.0.is_err());
        assert!(result.1.is_err());
        assert!(result.2.is_err());
    }
}
