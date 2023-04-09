use std::{collections::HashSet, rc::Rc};

use actix_utils::future::ok;
use actix_web::{
    body::{EitherBody, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse},
    http::{
        header::{self, HeaderValue},
        Method,
    },
    Error, HttpResponse, Result,
};
use futures_util::future::{FutureExt as _, LocalBoxFuture};
use log::debug;

use crate::{
    builder::intersperse_header_values,
    inner::{add_vary_header, header_value_try_into_method},
    AllOrSome, CorsError, Inner,
};

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
    /// Returns true if request is `OPTIONS` and contains an `Access-Control-Request-Method` header.
    fn is_request_preflight(req: &ServiceRequest) -> bool {
        // check request method is OPTIONS
        if req.method() != Method::OPTIONS {
            return false;
        }

        // check follow-up request method is present and valid
        if req
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(header_value_try_into_method)
            .is_none()
        {
            return false;
        }

        true
    }

    /// Validates preflight request headers against configuration and constructs preflight response.
    ///
    /// Checks:
    /// - `Origin` header is acceptable;
    /// - `Access-Control-Request-Method` header is acceptable;
    /// - `Access-Control-Request-Headers` header is acceptable.
    fn handle_preflight(&self, req: ServiceRequest) -> ServiceResponse {
        let inner = Rc::clone(&self.inner);

        match inner.validate_origin(req.head()) {
            Ok(true) => {}
            Ok(false) => return req.error_response(CorsError::OriginNotAllowed),
            Err(err) => return req.error_response(err),
        };

        if let Err(err) = inner
            .validate_allowed_method(req.head())
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

        #[cfg(feature = "draft-private-network-access")]
        if inner.allow_private_network_access
            && req
                .headers()
                .contains_key("access-control-request-private-network")
        {
            res.insert_header((
                header::HeaderName::from_static("access-control-allow-private-network"),
                HeaderValue::from_static("true"),
            ));
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

        let mut res = res.finish();

        if inner.vary_header {
            add_vary_header(res.headers_mut());
        }

        req.into_response(res)
    }

    fn augment_response<B>(
        inner: &Inner,
        origin_allowed: bool,
        mut res: ServiceResponse<B>,
    ) -> ServiceResponse<B> {
        if origin_allowed {
            if let Some(origin) = inner.access_control_allow_origin(res.request().head()) {
                res.headers_mut()
                    .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            };
        }

        if let Some(ref expose) = inner.expose_headers_baked {
            log::trace!("exposing selected headers: {:?}", expose);

            res.headers_mut()
                .insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, expose.clone());
        } else if matches!(inner.expose_headers, AllOrSome::All) {
            // intersperse_header_values requires that argument is non-empty
            if !res.headers().is_empty() {
                // extract header names from request
                let expose_all_request_headers = res
                    .headers()
                    .keys()
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

        #[cfg(feature = "draft-private-network-access")]
        if inner.allow_private_network_access
            && res
                .request()
                .headers()
                .contains_key("access-control-request-private-network")
        {
            res.headers_mut().insert(
                header::HeaderName::from_static("access-control-allow-private-network"),
                HeaderValue::from_static("true"),
            );
        }

        if inner.vary_header {
            add_vary_header(res.headers_mut());
        }

        res
    }
}

impl<S, B> Service<ServiceRequest> for CorsMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,

    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<ServiceResponse<EitherBody<B>>, Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let origin = req.headers().get(header::ORIGIN);

        // handle preflight requests
        if self.inner.preflight && Self::is_request_preflight(&req) {
            let res = self.handle_preflight(req);
            return ok(res.map_into_right_body()).boxed_local();
        }

        // only check actual requests with a origin header
        let origin_allowed = match (origin, self.inner.validate_origin(req.head())) {
            (None, _) => false,
            (_, Ok(origin_allowed)) => origin_allowed,
            (_, Err(err)) => {
                debug!("origin validation failed; inner service is not called");
                let mut res = req.error_response(err);

                if self.inner.vary_header {
                    add_vary_header(res.headers_mut());
                }

                return ok(res.map_into_right_body()).boxed_local();
            }
        };

        let inner = Rc::clone(&self.inner);
        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await;
            Ok(Self::augment_response(&inner, origin_allowed, res?).map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{
        dev::Transform,
        middleware::Compat,
        test::{self, TestRequest},
        App,
    };

    use super::*;
    use crate::Cors;

    #[test]
    fn compat_compat() {
        let _ = App::new().wrap(Compat::new(Cors::default()));
    }

    #[actix_web::test]
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
