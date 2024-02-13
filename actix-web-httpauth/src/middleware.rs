//! HTTP Authentication middleware.

use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};

use actix_web::{
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, FromRequest,
};
use futures_core::ready;
use futures_util::future::{self, LocalBoxFuture, TryFutureExt as _};

use crate::extractors::{api_key, basic, bearer};

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
    T: FromRequest,
{
    process_fn: Arc<F>,
    _extractor: PhantomData<T>,
}

impl<T, F, O> HttpAuthentication<T, F>
where
    T: FromRequest,
    F: Fn(ServiceRequest, T) -> O,
    O: Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>>,
{
    /// Construct `HttpAuthentication` middleware with the provided auth extractor `T` and
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
    O: Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Basic" authentication scheme.
    ///
    /// # Examples
    /// ```
    /// # use actix_web::{Error, dev::ServiceRequest};
    /// # use actix_web_httpauth::{extractors::basic::BasicAuth, middleware::HttpAuthentication};
    /// // In this example validator returns immediately, but since it is required to return
    /// // anything that implements `IntoFuture` trait, it can be extended to query database or to
    /// // do something else in a async manner.
    /// async fn validator(
    ///     req: ServiceRequest,
    ///     credentials: BasicAuth,
    /// ) -> Result<ServiceRequest, (Error, ServiceRequest)> {
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
    O: Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Bearer" authentication scheme.
    ///
    /// # Examples
    /// ```
    /// # use actix_web::{Error, dev::ServiceRequest};
    /// # use actix_web_httpauth::{
    /// #     extractors::{AuthenticationError, AuthExtractorConfig, bearer::{self, BearerAuth}},
    /// #     middleware::HttpAuthentication,
    /// # };
    /// async fn validator(
    ///     req: ServiceRequest,
    ///     credentials: BearerAuth
    /// ) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    ///     if credentials.token() == "mF_9.B5f-4.1JqM" {
    ///         Ok(req)
    ///     } else {
    ///         let config = req.app_data::<bearer::Config>()
    ///             .cloned()
    ///             .unwrap_or_default()
    ///             .scope("urn:example:channel=HBO&urn:example:rating=G,PG-13");
    ///
    ///         Err((AuthenticationError::from(config).into(), req))
    ///     }
    /// }
    ///
    /// let middleware = HttpAuthentication::bearer(validator);
    /// ```
    pub fn bearer(process_fn: F) -> Self {
        Self::with_fn(process_fn)
    }
}

impl<F, O> HttpAuthentication<api_key::APIKeyAuth, F>
where
    F: Fn(ServiceRequest, api_key::APIKeyAuth) -> O,
    O: Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Basic" authentication scheme.
    ///
    /// # Examples
    /// ```
    /// # use actix_web::{Error, dev::ServiceRequest};
    /// # use actix_web_httpauth::{extractors::basic::BasicAuth, middleware::HttpAuthentication};
    /// // In this example validator returns immediately, but since it is required to return
    /// // anything that implements `IntoFuture` trait, it can be extended to query database or to
    /// // do something else in a async manner.
    /// async fn validator(
    ///     req: ServiceRequest,
    ///     credentials: BasicAuth,
    /// ) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    ///     // All users are great and more than welcome!
    ///     Ok(req)
    /// }
    ///
    /// let middleware = HttpAuthentication::basic(validator);
    /// ```
    pub fn api_key(process_fn: F) -> Self {
        Self::with_fn(process_fn)
    }
}

impl<S, B, T, F, O> Transform<S, ServiceRequest> for HttpAuthentication<T, F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + 'static,
    O: Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>> + 'static,
    T: FromRequest + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = AuthenticationMiddleware<S, F, T>;
    type InitError = ();
    type Future = future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(AuthenticationMiddleware {
            service: Rc::new(service),
            process_fn: self.process_fn.clone(),
            _extractor: PhantomData,
        })
    }
}

#[doc(hidden)]
pub struct AuthenticationMiddleware<S, F, T>
where
    T: FromRequest,
{
    service: Rc<S>,
    process_fn: Arc<F>,
    _extractor: PhantomData<T>,
}

impl<S, B, F, T, O> Service<ServiceRequest> for AuthenticationMiddleware<S, F, T>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + 'static,
    O: Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>> + 'static,
    T: FromRequest + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let process_fn = Arc::clone(&self.process_fn);
        let service = Rc::clone(&self.service);

        Box::pin(async move {
            let (req, credentials) = match Extract::<T>::new(req).await {
                Ok(req) => req,
                Err((err, req)) => {
                    return Ok(req.error_response(err).map_into_right_body());
                }
            };

            let req = match process_fn(req, credentials).await {
                Ok(req) => req,
                Err((err, req)) => {
                    return Ok(req.error_response(err).map_into_right_body());
                }
            };

            service.call(req).await.map(|res| res.map_into_left_body())
        })
    }
}

struct Extract<T> {
    req: Option<ServiceRequest>,
    fut: Option<LocalBoxFuture<'static, Result<T, Error>>>,
    _extractor: PhantomData<fn() -> T>,
}

impl<T> Extract<T> {
    pub fn new(req: ServiceRequest) -> Self {
        Extract {
            req: Some(req),
            fut: None,
            _extractor: PhantomData,
        }
    }
}

impl<T> Future for Extract<T>
where
    T: FromRequest,
    T::Future: 'static,
    T::Error: 'static,
{
    type Output = Result<(ServiceRequest, T), (Error, ServiceRequest)>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.fut.is_none() {
            let req = self.req.as_mut().expect("Extract future was polled twice!");
            let fut = req.extract::<T>().map_err(Into::into);
            self.fut = Some(Box::pin(fut));
        }

        let fut = self
            .fut
            .as_mut()
            .expect("Extraction future should be initialized at this point");

        let credentials = ready!(fut.as_mut().poll(ctx)).map_err(|err| {
            (
                err,
                // returning request allows a proper error response to be created
                self.req.take().expect("Extract future was polled twice!"),
            )
        })?;

        let req = self.req.take().expect("Extract future was polled twice!");
        Poll::Ready(Ok((req, credentials)))
    }
}

#[cfg(test)]
mod tests {
    use actix_service::into_service;
    use actix_web::{
        dev::Service,
        error::{self, ErrorForbidden},
        http::StatusCode,
        test::TestRequest,
        web, App, HttpResponse,
    };

    use super::*;
    use crate::extractors::{basic::BasicAuth, bearer::BearerAuth};

    /// This is a test for https://github.com/actix/actix-extras/issues/10
    #[actix_web::test]
    async fn test_middleware_panic() {
        let middleware = AuthenticationMiddleware {
            service: Rc::new(into_service(|_: ServiceRequest| async move {
                actix_web::rt::time::sleep(std::time::Duration::from_secs(1)).await;
                Err::<ServiceResponse, _>(error::ErrorBadRequest("error"))
            })),
            process_fn: Arc::new(|req, _: BearerAuth| async { Ok(req) }),
            _extractor: PhantomData,
        };

        let req = TestRequest::get()
            .append_header(("Authorization", "Bearer 1"))
            .to_srv_request();

        let f = middleware.call(req).await;

        let _res = futures_util::future::lazy(|cx| middleware.poll_ready(cx)).await;

        assert!(f.is_err());
    }

    /// This is a test for https://github.com/actix/actix-extras/issues/10
    #[actix_web::test]
    async fn test_middleware_panic_several_orders() {
        let middleware = AuthenticationMiddleware {
            service: Rc::new(into_service(|_: ServiceRequest| async move {
                actix_web::rt::time::sleep(std::time::Duration::from_secs(1)).await;
                Err::<ServiceResponse, _>(error::ErrorBadRequest("error"))
            })),
            process_fn: Arc::new(|req, _: BearerAuth| async { Ok(req) }),
            _extractor: PhantomData,
        };

        let req = TestRequest::get()
            .append_header(("Authorization", "Bearer 1"))
            .to_srv_request();

        let f1 = middleware.call(req).await;

        let req = TestRequest::get()
            .append_header(("Authorization", "Bearer 1"))
            .to_srv_request();

        let f2 = middleware.call(req).await;

        let req = TestRequest::get()
            .append_header(("Authorization", "Bearer 1"))
            .to_srv_request();

        let f3 = middleware.call(req).await;

        let _res = futures_util::future::lazy(|cx| middleware.poll_ready(cx)).await;

        assert!(f1.is_err());
        assert!(f2.is_err());
        assert!(f3.is_err());
    }

    #[actix_web::test]
    async fn test_middleware_opt_extractor() {
        let middleware = AuthenticationMiddleware {
            service: Rc::new(into_service(|req: ServiceRequest| async move {
                Ok::<ServiceResponse, _>(req.into_response(HttpResponse::Ok().finish()))
            })),
            process_fn: Arc::new(|req, auth: Option<BearerAuth>| {
                assert!(auth.is_none());
                async { Ok(req) }
            }),
            _extractor: PhantomData,
        };

        let req = TestRequest::get()
            .append_header(("Authorization996", "Bearer 1"))
            .to_srv_request();

        let f = middleware.call(req).await;

        let _res = futures_util::future::lazy(|cx| middleware.poll_ready(cx)).await;

        assert!(f.is_ok());
    }

    #[actix_web::test]
    async fn test_middleware_res_extractor() {
        let middleware = AuthenticationMiddleware {
            service: Rc::new(into_service(|req: ServiceRequest| async move {
                Ok::<ServiceResponse, _>(req.into_response(HttpResponse::Ok().finish()))
            })),
            process_fn: Arc::new(
                |req, auth: Result<BearerAuth, <BearerAuth as FromRequest>::Error>| {
                    assert!(auth.is_err());
                    async { Ok(req) }
                },
            ),
            _extractor: PhantomData,
        };

        let req = TestRequest::get()
            .append_header(("Authorization", "BearerLOL"))
            .to_srv_request();

        let f = middleware.call(req).await;

        let _res = futures_util::future::lazy(|cx| middleware.poll_ready(cx)).await;

        assert!(f.is_ok());
    }

    #[actix_web::test]
    async fn test_middleware_works_with_app() {
        async fn validator(
            req: ServiceRequest,
            _credentials: BasicAuth,
        ) -> Result<ServiceRequest, (actix_web::Error, ServiceRequest)> {
            Err((ErrorForbidden("You are not welcome!"), req))
        }
        let middleware = HttpAuthentication::basic(validator);

        let srv = actix_web::test::init_service(
            App::new()
                .wrap(middleware)
                .route("/", web::get().to(HttpResponse::Ok)),
        )
        .await;

        let req = actix_web::test::TestRequest::with_uri("/")
            .append_header(("Authorization", "Basic DontCare"))
            .to_request();

        let resp = srv.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_middleware_works_with_scope() {
        async fn validator(
            req: ServiceRequest,
            _credentials: BasicAuth,
        ) -> Result<ServiceRequest, (actix_web::Error, ServiceRequest)> {
            Err((ErrorForbidden("You are not welcome!"), req))
        }
        let middleware = actix_web::middleware::Compat::new(HttpAuthentication::basic(validator));

        let srv = actix_web::test::init_service(
            App::new().service(
                web::scope("/")
                    .wrap(middleware)
                    .route("/", web::get().to(HttpResponse::Ok)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::with_uri("/")
            .append_header(("Authorization", "Basic DontCare"))
            .to_request();

        let resp = srv.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
