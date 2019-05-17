//! HTTP Authentication middleware.

use std::marker::PhantomData;
use std::rc::Rc;

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures::future::{self, FutureResult};
use futures::{Async, Future, IntoFuture, Poll};

use crate::extractors::{basic, bearer, AuthExtractor};

/// Middleware for checking HTTP authentication.
///
/// If there is no `Authorization` header in the request,
/// this middleware returns an error immediately,
/// without calling the `F` callback.
/// Otherwise, it will pass parsed credentials into it.
pub struct HttpAuthentication<T, F>
where
    T: AuthExtractor,
{
    validator_fn: Rc<F>,
    _extractor: PhantomData<T>,
}

impl<T, F, O> HttpAuthentication<T, F>
where
    T: AuthExtractor,
    F: FnMut(&mut ServiceRequest, T) -> O,
    O: IntoFuture<Item = (), Error = Error>,
{
    /// Construct `HttpAuthentication` middleware
    /// with the provided auth extractor `T` and
    /// validation callback `F`.
    pub fn with_fn(validator_fn: F) -> HttpAuthentication<T, F> {
        HttpAuthentication {
            validator_fn: Rc::new(validator_fn),
            _extractor: PhantomData,
        }
    }
}

impl<F, O> HttpAuthentication<basic::BasicAuth, F>
where
    F: FnMut(&mut ServiceRequest, basic::BasicAuth) -> O,
    O: IntoFuture<Item = (), Error = Error>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Basic"
    /// authentication scheme.
    ///
    /// ## Example
    ///
    /// ```
    /// # use actix_web::Error;
    /// # use actix_web::dev::ServiceRequest;
    /// # use futures::future::{self, FutureResult};
    /// # use actix_web_httpauth::middleware::HttpAuthentication;
    /// # use actix_web_httpauth::extractors::basic::BasicAuth;
    /// // In this example validator returns immediately,
    /// // but since it is required to return anything
    /// // that implements `IntoFuture` trait,
    /// // it can be extended to query database
    /// // or to do something else in a async manner.
    /// fn validator(
    ///     req: &mut ServiceRequest,
    ///     credentials: BasicAuth,
    /// ) -> FutureResult<(), Error> {
    ///     // All users are great and more than welcome!
    ///     future::ok(())
    /// }
    ///
    /// let middleware = HttpAuthentication::basic(validator);
    /// ```
    pub fn basic(validator_fn: F) -> Self {
        Self::with_fn(validator_fn)
    }
}

impl<F, O> HttpAuthentication<bearer::BearerAuth, F>
where
    F: FnMut(&mut ServiceRequest, bearer::BearerAuth) -> O,
    O: IntoFuture<Item = (), Error = Error>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Bearer"
    /// authentication scheme.
    /// ## Example
    ///
    /// ```
    /// # use actix_web::Error;
    /// # use actix_web::dev::ServiceRequest;
    /// # use futures::future::{self, FutureResult};
    /// # use actix_web_httpauth::middleware::HttpAuthentication;
    /// # use actix_web_httpauth::extractors::bearer::{Config, BearerAuth};
    /// # use actix_web_httpauth::extractors::{AuthenticationError, AuthExtractorConfig};
    /// fn validator(req: &mut ServiceRequest, credentials: BearerAuth) -> FutureResult<(), Error> {
    ///     if credentials.token() == "mF_9.B5f-4.1JqM" {
    ///         future::ok(())
    ///     } else {
    ///         let config = req.app_data::<Config>()
    ///             .map(|data| data.get_ref().clone())
    ///             .unwrap_or_else(Default::default)
    ///             .scope("urn:example:channel=HBO&urn:example:rating=G,PG-13");
    ///
    ///         future::err(AuthenticationError::from(config).into())
    ///     }
    /// }
    ///
    /// let middleware = HttpAuthentication::bearer(validator);
    /// ```
    pub fn bearer(validator_fn: F) -> Self {
        Self::with_fn(validator_fn)
    }
}

impl<S, B, T, F, O> Transform<S> for HttpAuthentication<T, F>
where
    S: Service<
            Request = ServiceRequest,
            Response = ServiceResponse<B>,
            Error = Error,
        > + 'static,
    S::Future: 'static,
    F: Fn(&mut ServiceRequest, T) -> O + 'static,
    O: IntoFuture<Item = (), Error = Error> + 'static,
    T: AuthExtractor + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthenticationMiddleware<S, F, T>;
    type InitError = ();
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(AuthenticationMiddleware {
            service: Some(service),
            validator_fn: self.validator_fn.clone(),
            _extractor: PhantomData,
        })
    }
}

#[doc(hidden)]
pub struct AuthenticationMiddleware<S, F, T>
where
    T: AuthExtractor,
{
    service: Option<S>,
    validator_fn: Rc<F>,
    _extractor: PhantomData<T>,
}

impl<S, B, F, T, O> Service for AuthenticationMiddleware<S, F, T>
where
    S: Service<
            Request = ServiceRequest,
            Response = ServiceResponse<B>,
            Error = Error,
        > + 'static,
    S::Future: 'static,
    F: Fn(&mut ServiceRequest, T) -> O + 'static,
    O: IntoFuture<Item = (), Error = Error> + 'static,
    T: AuthExtractor + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future = Box<dyn Future<Item = ServiceResponse<B>, Error = Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service
            .as_mut()
            .expect("AuthenticationMiddleware was called already")
            .poll_ready()
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let validator_fn = self.validator_fn.clone();
        let mut service = self
            .service
            .take()
            .expect("AuthenticationMiddleware was called twice");

        let f = Extract::new(req)
            .and_then(move |(req, credentials)| {
                Validate::new(req, validator_fn, credentials)
            })
            .and_then(move |req| service.call(req));

        Box::new(f)
    }
}

struct Extract<T> {
    req: Option<ServiceRequest>,
    f: Option<Box<dyn Future<Item = T, Error = Error>>>,
    _extractor: PhantomData<T>,
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
    type Item = (ServiceRequest, T);
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.f.is_none() {
            let req =
                self.req.as_ref().expect("Extract future was polled twice!");
            let f = T::from_service_request(req)
                .into_future()
                .map_err(Into::into);
            self.f = Some(Box::new(f));
        }

        let f = self
            .f
            .as_mut()
            .expect("Extraction future should be initialized at this point");
        let credentials = futures::try_ready!(f.poll());

        let req = self.req.take().expect("Extract future was polled twice!");
        Ok(Async::Ready((req, credentials)))
    }
}

struct Validate<F, T> {
    req: Option<ServiceRequest>,
    validation_f: Option<Box<dyn Future<Item = (), Error = Error>>>,
    validator_fn: Rc<F>,
    credentials: Option<T>,
}

impl<F, T> Validate<F, T> {
    pub fn new(
        req: ServiceRequest,
        validator_fn: Rc<F>,
        credentials: T,
    ) -> Self {
        Validate {
            req: Some(req),
            credentials: Some(credentials),
            validator_fn,
            validation_f: None,
        }
    }
}

impl<F, T, O> Future for Validate<F, T>
where
    F: Fn(&mut ServiceRequest, T) -> O,
    O: IntoFuture<Item = (), Error = Error> + 'static,
{
    type Item = ServiceRequest;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.validation_f.is_none() {
            let req = self
                .req
                .as_mut()
                .expect("Unable to get the mutable access to the request");
            let credentials = self
                .credentials
                .take()
                .expect("Validate future was polled in some weird manner");
            let f = (self.validator_fn)(req, credentials).into_future();

            self.validation_f = Some(Box::new(f));
        }

        let f = self
            .validation_f
            .as_mut()
            .expect("Validation future should exist at this moment");
        // We do not care about returned `Ok(())`
        futures::try_ready!(f.poll());
        let req = self.req.take().expect("Validate future was polled already");

        Ok(Async::Ready(req))
    }
}
