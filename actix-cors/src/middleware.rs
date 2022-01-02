use std::{collections::HashSet, convert::TryInto, rc::Rc};

use actix_utils::future::ok;
use actix_web::{
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse},
    http::{
        header::{self, HeaderValue},
        Method,
    },
    Error, HttpResponse, Result,
};
use futures_util::future::{FutureExt as _, LocalBoxFuture};
use log::debug;

use crate::{builder::intersperse_header_values, AllOrSome, Inner};

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
    fn handle_preflight(inner: &Inner, req: ServiceRequest) -> ServiceResponse {
        if let Err(err) = inner
            .validate_origin(req.head())
            .and_then(|_| inner.validate_allowed_method(req.head()))
            .and_then(|_| inner.validate_allowed_headers(req.head()))
        {
            return req.error_response(err);
        }

        let mut res = HttpResponse::Ok();

        if let Some(origin) = inner.access_control_allow_origin(req.head()) {
            res.insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, origin));
        }

        if let Some(ref allowed_methods) = inner.allowed_methods_baked {
            res.insert_header((
                header::ACCESS_CONTROL_ALLOW_METHODS,
                allowed_methods.clone(),
            ));
        }

        if let Some(ref headers) = inner.allowed_headers_baked {
            res.insert_header((header::ACCESS_CONTROL_ALLOW_HEADERS, headers.clone()));
        } else if let Some(headers) = req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS) {
            // all headers allowed, return
            res.insert_header((header::ACCESS_CONTROL_ALLOW_HEADERS, headers.clone()));
        }

        if inner.supports_credentials {
            res.insert_header((
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            ));
        }

        if let Some(max_age) = inner.max_age {
            res.insert_header((header::ACCESS_CONTROL_MAX_AGE, max_age.to_string()));
        }

        let res = res.finish();
        req.into_response(res)
    }

    fn augment_response<B>(inner: &Inner, mut res: ServiceResponse<B>) -> ServiceResponse<B> {
        if let Some(origin) = inner.access_control_allow_origin(res.request().head()) {
            res.headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        };

        if let Some(ref expose) = inner.expose_headers_baked {
            log::trace!("exposing selected headers: {:?}", expose);

            res.headers_mut()
                .insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, expose.clone());
        } else if matches!(inner.expose_headers, AllOrSome::All) {
            // intersperse_header_values requires that argument is non-empty
            if !res.request().headers().is_empty() {
                // extract header names from request
                let expose_all_request_headers = res
                    .request()
                    .headers()
                    .keys()
                    .into_iter()
                    .map(|name| name.as_str())
                    .collect::<HashSet<_>>();

                // create comma separated string of header names
                let expose_headers_value = intersperse_header_values(&expose_all_request_headers);

                log::trace!(
                    "exposing all headers from request: {:?}",
                    expose_headers_value
                );

                // add header names to expose response header
                res.headers_mut()
                    .insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, expose_headers_value);
            }
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
                    let mut val: Vec<u8> = Vec::with_capacity(hdr.len() + 71);
                    val.extend(hdr.as_bytes());
                    val.extend(
                        b", Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
                    );
                    val.try_into().unwrap()
                }
                None => HeaderValue::from_static(
                    "Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
                ),
            };

            res.headers_mut().insert(header::VARY, value);
        }

        res
    }
}

impl<S, B> Service<ServiceRequest> for CorsMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,

    B: MessageBody + 'static,
    B::Error: Into<Error>,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<ServiceResponse<EitherBody<B>>, Error>>;

    actix_service::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if self.inner.preflight && req.method() == Method::OPTIONS {
            let inner = Rc::clone(&self.inner);
            let res = Self::handle_preflight(&inner, req);
            ok(res.map_into_right_body()).boxed_local()
        } else {
            let origin = req.headers().get(header::ORIGIN).cloned();

            if origin.is_some() {
                // Only check requests with a origin header.
                if let Err(err) = self.inner.validate_origin(req.head()) {
                    debug!("origin validation failed; inner service is not called");
                    return ok(req.error_response(err).map_into_right_body()).boxed_local();
                }
            }

            let inner = Rc::clone(&self.inner);
            let fut = self.service.call(req);

            async move {
                let res = fut.await;

                if origin.is_some() {
                    Ok(Self::augment_response(&inner, res?))
                } else {
                    res.map_err(Into::into)
                }
                .map(|res| res.map_into_left_body())
            }
            .boxed_local()
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{
        dev::Transform,
        test::{self, TestRequest},
    };

    use super::*;
    use crate::Cors;

    #[actix_rt::test]
    async fn test_options_no_origin() {
        // Tests case where allowed_origins is All but there are validate functions to run incase.
        // In this case, origins are only allowed when the DNT header is sent.

        let cors = Cors::default()
            .allow_any_origin()
            .allowed_origin_fn(|origin, req_head| {
                assert_eq!(&origin, req_head.headers.get(header::ORIGIN).unwrap());

                req_head.headers().contains_key(header::DNT)
            })
            .new_transform(test::ok_service())
            .await
            .unwrap();

        let req = TestRequest::get()
            .insert_header((header::ORIGIN, "http://example.com"))
            .to_srv_request();
        let res = cors.call(req).await.unwrap();
        assert_eq!(
            None,
            res.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(HeaderValue::as_bytes)
        );

        let req = TestRequest::get()
            .insert_header((header::ORIGIN, "http://example.com"))
            .insert_header((header::DNT, "1"))
            .to_srv_request();
        let res = cors.call(req).await.unwrap();
        assert_eq!(
            Some(&b"http://example.com"[..]),
            res.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(HeaderValue::as_bytes)
        );
    }
}
