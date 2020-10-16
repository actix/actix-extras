use std::{
    convert::TryInto,
    rc::Rc,
    task::{Context, Poll},
};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse},
    error::{Error, Result},
    http::{
        header::{self, HeaderValue},
        Method,
    },
    HttpResponse,
};
use futures_util::future::{ok, Either, FutureExt as _, LocalBoxFuture, Ready};

use crate::Inner;

/// Service wrapper for Cross-Origin Resource Sharing support.
///
/// This struct contains the settings for CORS requests to be validated and for responses to
/// be generated.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct CorsMiddleware<S> {
    pub(crate) service: S,
    pub(crate) inner: Rc<Inner>,
}

type CorsMiddlewareServiceFuture<B> = Either<
    Ready<Result<ServiceResponse<B>, Error>>,
    LocalBoxFuture<'static, Result<ServiceResponse<B>, Error>>,
>;

impl<S, B> Service for CorsMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = CorsMiddlewareServiceFuture<B>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        if self.inner.preflight && Method::OPTIONS == *req.method() {
            if let Err(e) = self
                .inner
                .validate_origin(req.head())
                .and_then(|_| self.inner.validate_allowed_method(req.head()))
                .and_then(|_| self.inner.validate_allowed_headers(req.head()))
            {
                return Either::Left(ok(req.error_response(e)));
            }

            // allowed headers
            let headers = if let Some(headers) = self.inner.headers.as_ref() {
                Some(
                    headers
                        .iter()
                        .fold(String::new(), |s, v| s + "," + v.as_str())
                        .as_str()[1..]
                        .try_into()
                        .unwrap(),
                )
            } else if let Some(hdr) =
                req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            {
                Some(hdr.clone())
            } else {
                None
            };

            let res = HttpResponse::Ok()
                .if_some(self.inner.max_age.as_ref(), |max_age, resp| {
                    let _ = resp.header(
                        header::ACCESS_CONTROL_MAX_AGE,
                        format!("{}", max_age).as_str(),
                    );
                })
                .if_some(headers, |headers, resp| {
                    let _ = resp.header(header::ACCESS_CONTROL_ALLOW_HEADERS, headers);
                })
                .if_some(
                    self.inner.access_control_allow_origin(req.head()),
                    |origin, resp| {
                        let _ = resp.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
                    },
                )
                .if_true(self.inner.supports_credentials, |resp| {
                    resp.header(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true");
                })
                .header(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    &self
                        .inner
                        .methods
                        .iter()
                        .fold(String::new(), |s, v| s + "," + v.as_str())
                        .as_str()[1..],
                )
                .finish()
                .into_body();

            Either::Left(ok(req.into_response(res)))
        } else {
            if req.headers().contains_key(header::ORIGIN) {
                // Only check requests with a origin header.
                if let Err(e) = self.inner.validate_origin(req.head()) {
                    return Either::Left(ok(req.error_response(e)));
                }
            }

            let inner = Rc::clone(&self.inner);
            let has_origin = req.headers().contains_key(header::ORIGIN);
            let fut = self.service.call(req);

            Either::Right(
                async move {
                    let res = fut.await;

                    if has_origin {
                        let mut res = res?;
                        if let Some(origin) =
                            inner.access_control_allow_origin(res.request().head())
                        {
                            res.headers_mut()
                                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
                        };

                        if let Some(ref expose) = inner.expose_headers {
                            res.headers_mut().insert(
                                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                                expose.as_str().try_into().unwrap(),
                            );
                        }
                        if inner.supports_credentials {
                            res.headers_mut().insert(
                                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                                HeaderValue::from_static("true"),
                            );
                        }
                        if inner.vary_header {
                            let value =
                                if let Some(hdr) = res.headers_mut().get(header::VARY) {
                                    let mut val: Vec<u8> =
                                        Vec::with_capacity(hdr.as_bytes().len() + 8);
                                    val.extend(hdr.as_bytes());
                                    val.extend(b", Origin");
                                    val.try_into().unwrap()
                                } else {
                                    HeaderValue::from_static("Origin")
                                };
                            res.headers_mut().insert(header::VARY, value);
                        }
                        Ok(res)
                    } else {
                        res
                    }
                }
                .boxed_local(),
            )
        }
    }
}
