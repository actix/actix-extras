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
use log::debug;

use crate::{AllOrSome, Inner};

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

impl<S> CorsMiddleware<S> {
    fn handle_preflight<B>(inner: &Inner, req: ServiceRequest) -> ServiceResponse<B> {
        if let Err(err) = inner
            .validate_origin(req.head())
            .and_then(|_| inner.validate_allowed_method(req.head()))
            .and_then(|_| inner.validate_allowed_headers(req.head()))
        {
            return req.error_response(err);
        }

        let allowed_headers = if let Some(headers) = inner.allowed_headers.as_ref() {
            let header_list = &headers
                .iter()
                .fold(String::new(), |s, v| s + "," + v.as_str());

            Some(HeaderValue::from_str(&header_list[1..]).unwrap())
        } else if let Some(hdr) =
            req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        {
            Some(hdr.clone())
        } else {
            None
        };

        let allowed_methods = inner
            .methods
            .iter()
            .fold(String::new(), |s, v| s + "," + v.as_str());

        let mut res = HttpResponse::Ok();

        if let Some(max_age) = inner.max_age {
            res.header(header::ACCESS_CONTROL_MAX_AGE, max_age.to_string());
        }

        if let Some(headers) = allowed_headers {
            res.header(header::ACCESS_CONTROL_ALLOW_HEADERS, headers);
        }

        if let Some(origin) = inner.access_control_allow_origin(req.head()) {
            res.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }

        if inner.supports_credentials {
            res.header(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true");
        }

        res.header(header::ACCESS_CONTROL_ALLOW_METHODS, &allowed_methods[1..]);

        let res = res.finish();
        let res = res.into_body();
        let res = req.into_response(res);

        res
    }

    fn augment_response<B>(
        inner: &Inner,
        mut res: ServiceResponse<B>,
    ) -> Result<ServiceResponse<B>, Error> {
        if let Some(origin) = inner.access_control_allow_origin(res.request().head()) {
            res.headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        };

        if let AllOrSome::Some(ref expose) = inner.expose_headers {
            let expose_str = expose
                .iter()
                .map(|hdr| hdr.to_string())
                .collect::<Vec<_>>()
                .join(", ")
                .try_into()
                .unwrap();

            res.headers_mut()
                .insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, expose_str);
        }

        if inner.supports_credentials {
            res.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
        }

        if inner.vary_header {
            let value = match res.headers_mut().get(header::VARY) {
                Some(hdr) => {
                    let mut val: Vec<u8> = Vec::with_capacity(hdr.len() + 8);
                    val.extend(hdr.as_bytes());
                    val.extend(b", Origin");
                    val.try_into().unwrap()
                }
                None => HeaderValue::from_static("Origin"),
            };

            res.headers_mut().insert(header::VARY, value);
        }

        Ok(res)
    }
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
        if self.inner.preflight && req.method() == Method::OPTIONS {
            let inner = Rc::clone(&self.inner);
            let res = Self::handle_preflight(&inner, req);
            Either::Left(ok(res))
        } else {
            let origin = req.headers().get(header::ORIGIN).cloned();

            if origin.is_some() {
                // Only check requests with a origin header.
                if let Err(err) = self.inner.validate_origin(req.head()) {
                    debug!("origin validation failed; inner service is not called");
                    return Either::Left(ok(req.error_response(err)));
                }
            }

            let inner = Rc::clone(&self.inner);
            let fut = self.service.call(req);

            let res = async move {
                let res = fut.await;

                if origin.is_some() {
                    let res = res?;
                    Self::augment_response(&inner, res)
                } else {
                    res
                }
            }
            .boxed_local();

            Either::Right(res)
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{dev::Transform, http::Method, test};

    use super::*;
    use crate::Cors;

    #[actix_rt::test]
    async fn test_options_no_origin() {
        let mut cors = Cors::default()
            // .allowed_origin("http://localhost:8080")
            .new_transform(test::ok_service())
            .await
            .unwrap();

        let req = test::TestRequest::default()
            .method(Method::OPTIONS)
            .to_srv_request();

        cors.call(req).await.unwrap();
    }
}
