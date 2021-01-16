//! HTTP Authentication middleware.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures_core::{future::LocalBoxFuture, ready};

use crate::extractors::{basic, bearer, AuthExtractor};

/// Middleware for checking HTTP authentication.
///
/// If there is no `Authorization` header in the request, this middleware returns an error
/// immediately, without calling the `F` callback.
///
/// Otherwise, it will pass both the request and the parsed credentials into it. In case of
/// successful validation `F` callback is required to return the `ServiceRequest` back.
#[derive(Debug, Clone)]
pub struct HttpAuthentication<T, F>
where
    T: AuthExtractor,
{
    process_fn: F,
    _extractor: PhantomData<T>,
}

impl<T, F, O> HttpAuthentication<T, F>
where
    T: AuthExtractor,
    F: Fn(ServiceRequest, T) -> O + Clone,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware with the provided auth extractor `T` and
    /// validation callback `F`.
    pub fn with_fn(process_fn: F) -> HttpAuthentication<T, F> {
        HttpAuthentication {
            process_fn,
            _extractor: PhantomData,
        }
    }
}

impl<F, O> HttpAuthentication<basic::BasicAuth, F>
where
    F: Fn(ServiceRequest, basic::BasicAuth) -> O + Clone,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Basic" authentication scheme.
    ///
    /// # Example
    ///
    /// ```
    /// # use actix_web::Error;
    /// # use actix_web::dev::ServiceRequest;
    /// # use actix_web_httpauth::middleware::HttpAuthentication;
    /// # use actix_web_httpauth::extractors::basic::BasicAuth;
    /// // In this example validator returns immediately, but since it is required to return
    /// // anything that implements `IntoFuture` trait, it can be extended to query database or to
    /// // do something else in a async manner.
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
    F: Fn(ServiceRequest, bearer::BearerAuth) -> O + Clone,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Bearer" authentication scheme.
    ///
    /// # Example
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

impl<S, B, T, F, O> Transform<S, ServiceRequest> for HttpAuthentication<T, F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + Clone + 'static,
    O: Future<Output = Result<ServiceRequest, Error>> + 'static,
    T: AuthExtractor + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthenticationMiddleware<S, F, T>;
    type InitError = ();
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let process_fn = self.process_fn.clone();
        Box::pin(async move {
            Ok(AuthenticationMiddleware {
                service: Rc::new(service),
                process_fn,
                _extractor: PhantomData,
            })
        })
    }
}

#[doc(hidden)]
pub struct AuthenticationMiddleware<S, F, T> {
    service: Rc<S>,
    process_fn: F,
    _extractor: PhantomData<T>,
}

impl<S, B, F, T, O> Service<ServiceRequest> for AuthenticationMiddleware<S, F, T>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + Clone + 'static,
    O: Future<Output = Result<ServiceRequest, Error>> + 'static,
    T: AuthExtractor + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future = AuthFuture<S, B, F, T, O>;

    actix_service::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        AuthFuture::Extract {
            extract: Extract::<T>::new(req),
            func: self.process_fn.clone(),
            service: Some(Rc::clone(&self.service)),
        }
    }
}

pin_project_lite::pin_project! {
    #[project = AuthFutureProj]
    #[doc(hidden)]
    pub enum AuthFuture<S, B, F, T, O>
    where
        T: AuthExtractor,
        F: Fn(ServiceRequest, T) -> O,
        O: Future,
        S: Service<ServiceRequest, Response = ServiceResponse<B>>
    {
        Extract {
            #[pin]
            extract: Extract<T>,
            func: F,
            service: Option<Rc<S>>,
        },
        Process {
            #[pin]
            fut: O,
            service: Rc<S>,
        },
        ServiceCall {
            #[pin]
            fut: S::Future
        }
    }
}

impl<S, B, F, T, O> Future for AuthFuture<S, B, F, T, O>
where
    T: AuthExtractor,
    T: 'static,
    F: Fn(ServiceRequest, T) -> O,
    O: Future<Output = Result<ServiceRequest, Error>>,
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Output = Result<ServiceResponse<B>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.as_mut().project() {
            AuthFutureProj::Extract {
                extract,
                func,
                service,
            } => {
                let (req, credentials) = ready!(extract.poll(cx))?;
                let fut = func(req, credentials);
                let service = service.take().expect("Future polled after finish");
                self.as_mut().set(AuthFuture::Process { fut, service });
                self.poll(cx)
            }
            AuthFutureProj::Process { fut, service } => {
                let req = ready!(fut.poll(cx))?;
                let fut = service.call(req);
                self.as_mut().set(AuthFuture::ServiceCall { fut });
                self.poll(cx)
            }
            AuthFutureProj::ServiceCall { fut } => fut.poll(cx),
        }
    }
}

pin_project_lite::pin_project! {
    #[doc(hidden)]
    pub struct Extract<T: AuthExtractor> {
        req: Option<ServiceRequest>,
        #[pin]
        fut: T::Future,
    }
}

impl<T: AuthExtractor> Extract<T> {
    fn new(req: ServiceRequest) -> Self {
        let fut = T::from_service_request(&req);
        Extract {
            req: Some(req),
            fut,
        }
    }
}

impl<T: AuthExtractor> Future for Extract<T> {
    type Output = Result<(ServiceRequest, T), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let credentials = ready!(this.fut.poll(cx)).map_err(Into::into)?;
        let req = this
            .req
            .take()
            .expect("Extract future was polled after finish!");
        Poll::Ready(Ok((req, credentials)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::bearer::BearerAuth;
    use actix_service::{into_service, Service};
    use actix_web::http::StatusCode;
    use actix_web::test::{init_service, TestRequest};
    use actix_web::{error, web, App};
    use futures_util::join;

    /// This is a test for https://github.com/actix/actix-extras/issues/10
    #[actix_rt::test]
    async fn test_middleware_panic() {
        let middleware = AuthenticationMiddleware {
            service: Rc::new(into_service(|_: ServiceRequest| async move {
                actix_rt::time::sleep(std::time::Duration::from_secs(1)).await;
                Err::<ServiceResponse, _>(error::ErrorBadRequest("error"))
            })),
            process_fn: |req, _: BearerAuth| async { Ok(req) },
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
        let middleware = AuthenticationMiddleware {
            service: Rc::new(into_service(|_: ServiceRequest| async move {
                actix_rt::time::sleep(std::time::Duration::from_secs(1)).await;
                Err::<ServiceResponse, _>(error::ErrorBadRequest("error"))
            })),
            process_fn: |req, _: BearerAuth| async { Ok(req) },
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

    #[actix_rt::test]
    async fn test_middleware_closure_clone() {
        let state = std::rc::Rc::new(321usize);
        let auth = HttpAuthentication::bearer(move |req, _| {
            let state = state.clone();
            async move {
                assert_eq!(321usize, *state);
                Ok(req)
            }
        });

        let app = init_service(
            App::new()
                .wrap(auth.clone())
                .service(web::resource("/test").to(|| async { "ok" })),
        )
        .await;

        let req = TestRequest::with_uri("/test")
            .header("Authorization", "Bearer 1")
            .to_request();

        let res = app.call(req).await.unwrap();
        assert_eq!(StatusCode::OK, res.status());
    }

    #[test]
    fn test_middleware_closure_thread_safety() {
        let state = std::sync::Arc::new(321usize);
        let auth = HttpAuthentication::bearer(move |req, _| {
            let state = state.clone();
            async move {
                assert_eq!(321usize, *state);
                Ok(req)
            }
        });

        let auth_clone = auth.clone();
        std::thread::spawn(move || {
            actix_rt::System::new("test_middleware").block_on(async {
                let app = init_service(
                    App::new()
                        .wrap(auth_clone)
                        .service(web::resource("/test").to(|| async { "ok" })),
                )
                .await;

                let req = TestRequest::with_uri("/test")
                    .header("Authorization", "Bearer 1")
                    .to_request();

                let res = app.call(req).await.unwrap();
                assert_eq!(StatusCode::OK, res.status());
            })
        })
        .join()
        .unwrap();

        drop(auth);
    }
}
