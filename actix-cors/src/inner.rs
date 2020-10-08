use std::{collections::HashSet, convert::TryInto, fmt};

use actix_web::{
    dev::RequestHead,
    error::Result,
    http::{
        self,
        header::{self, HeaderName, HeaderValue},
        Method,
    },
};

use crate::{AllOrSome, CorsError};

pub(crate) fn cors<'a>(
    parts: &'a mut Option<Inner>,
    err: &Option<http::Error>,
) -> Option<&'a mut Inner> {
    if err.is_some() {
        return None;
    }

    parts.as_mut()
}

pub(crate) struct OriginFn {
    pub(crate) boxed_fn: Box<dyn Fn(&RequestHead) -> bool>,
}

impl fmt::Debug for OriginFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("origin_fn")
    }
}

#[derive(Debug)]
pub(crate) struct Inner {
    pub(crate) methods: HashSet<Method>,
    pub(crate) origins: AllOrSome<HashSet<String>>,
    pub(crate) origins_fns: Vec<OriginFn>,
    pub(crate) origins_str: Option<HeaderValue>,
    pub(crate) headers: AllOrSome<HashSet<HeaderName>>,
    pub(crate) expose_headers: Option<String>,
    pub(crate) max_age: Option<usize>,
    pub(crate) preflight: bool,
    pub(crate) send_wildcard: bool,
    pub(crate) supports_credentials: bool,
    pub(crate) vary_header: bool,
}

impl Inner {
    pub(crate) fn validate_origin(&self, req: &RequestHead) -> Result<(), CorsError> {
        if let Some(hdr) = req.headers().get(header::ORIGIN) {
            if let Ok(origin) = hdr.to_str() {
                return match self.origins {
                    AllOrSome::All => Ok(()),
                    AllOrSome::Some(ref allowed_origins) => allowed_origins
                        .get(origin)
                        .map(|_| ())
                        .or_else(|| {
                            if self.validate_origin_fns(req) {
                                Some(())
                            } else {
                                None
                            }
                        })
                        .ok_or(CorsError::OriginNotAllowed),
                };
            }
            Err(CorsError::BadOrigin)
        } else {
            match self.origins {
                AllOrSome::All => Ok(()),
                _ => Err(CorsError::MissingOrigin),
            }
        }
    }

    pub(crate) fn validate_origin_fns(&self, req: &RequestHead) -> bool {
        self.origins_fns
            .iter()
            .any(|origin_fn| (origin_fn.boxed_fn)(req))
    }

    pub(crate) fn access_control_allow_origin(
        &self,
        req: &RequestHead,
    ) -> Option<HeaderValue> {
        match self.origins {
            AllOrSome::All => {
                if self.send_wildcard {
                    Some(HeaderValue::from_static("*"))
                } else if let Some(origin) = req.headers().get(header::ORIGIN) {
                    Some(origin.clone())
                } else {
                    None
                }
            }
            AllOrSome::Some(ref origins) => {
                if let Some(origin) =
                    req.headers()
                        .get(header::ORIGIN)
                        .filter(|o| match o.to_str() {
                            Ok(os) => origins.contains(os),
                            _ => false,
                        })
                {
                    Some(origin.clone())
                } else if self.validate_origin_fns(req) {
                    Some(req.headers().get(header::ORIGIN).unwrap().clone())
                } else {
                    Some(self.origins_str.as_ref().unwrap().clone())
                }
            }
        }
    }

    pub(crate) fn validate_allowed_method(
        &self,
        req: &RequestHead,
    ) -> Result<(), CorsError> {
        if let Some(hdr) = req.headers().get(header::ACCESS_CONTROL_REQUEST_METHOD) {
            if let Ok(meth) = hdr.to_str() {
                if let Ok(method) = meth.try_into() {
                    return self
                        .methods
                        .get(&method)
                        .map(|_| ())
                        .ok_or(CorsError::MethodNotAllowed);
                }
            }
            Err(CorsError::BadRequestMethod)
        } else {
            Err(CorsError::MissingRequestMethod)
        }
    }

    pub(crate) fn validate_allowed_headers(
        &self,
        req: &RequestHead,
    ) -> Result<(), CorsError> {
        match self.headers {
            AllOrSome::All => Ok(()),
            AllOrSome::Some(ref allowed_headers) => {
                if let Some(hdr) =
                    req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS)
                {
                    if let Ok(headers) = hdr.to_str() {
                        #[allow(clippy::mutable_key_type)] // FIXME: revisit here
                        let mut validated_headers = HashSet::new();
                        for hdr in headers.split(',') {
                            match hdr.trim().try_into() {
                                Ok(hdr) => validated_headers.insert(hdr),
                                Err(_) => return Err(CorsError::BadRequestHeaders),
                            };
                        }
                        // `Access-Control-Request-Headers` must contain 1 or more `field-name`
                        if !validated_headers.is_empty() {
                            if !validated_headers.is_subset(allowed_headers) {
                                return Err(CorsError::HeadersNotAllowed);
                            }
                            return Ok(());
                        }
                    }
                    Err(CorsError::BadRequestHeaders)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use actix_web::{
        dev::Transform,
        http::{header, Method, StatusCode},
        test::{self, TestRequest},
    };

    use crate::Cors;

    #[actix_rt::test]
    #[should_panic(expected = "OriginNotAllowed")]
    async fn test_validate_not_allowed_origin() {
        let cors = Cors::new()
            .allowed_origin("https://www.example.com")
            .finish()
            .new_transform(test::ok_service())
            .await
            .unwrap();

        let req = TestRequest::with_header("Origin", "https://www.unknown.com")
            .method(Method::GET)
            .to_srv_request();

        cors.inner.validate_origin(req.head()).unwrap();
        cors.inner.validate_allowed_method(req.head()).unwrap();
        cors.inner.validate_allowed_headers(req.head()).unwrap();
    }

    #[actix_rt::test]
    async fn test_preflight() {
        let mut cors = Cors::new()
            .send_wildcard()
            .max_age(3600)
            .allowed_methods(vec![Method::GET, Method::OPTIONS, Method::POST])
            .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
            .allowed_header(header::CONTENT_TYPE)
            .finish()
            .new_transform(test::ok_service())
            .await
            .unwrap();

        let req = TestRequest::with_header("Origin", "https://www.example.com")
            .method(Method::OPTIONS)
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "X-Not-Allowed")
            .to_srv_request();

        assert!(cors.inner.validate_allowed_method(req.head()).is_err());
        assert!(cors.inner.validate_allowed_headers(req.head()).is_err());
        let resp = test::call_service(&mut cors, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let req = TestRequest::with_header("Origin", "https://www.example.com")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "put")
            .method(Method::OPTIONS)
            .to_srv_request();

        assert!(cors.inner.validate_allowed_method(req.head()).is_err());
        assert!(cors.inner.validate_allowed_headers(req.head()).is_ok());

        let req = TestRequest::with_header("Origin", "https://www.example.com")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                "AUTHORIZATION,ACCEPT",
            )
            .method(Method::OPTIONS)
            .to_srv_request();

        let resp = test::call_service(&mut cors, req).await;
        assert_eq!(
            &b"*"[..],
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .as_bytes()
        );
        assert_eq!(
            &b"3600"[..],
            resp.headers()
                .get(header::ACCESS_CONTROL_MAX_AGE)
                .unwrap()
                .as_bytes()
        );
        let hdr = resp
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(hdr.contains("authorization"));
        assert!(hdr.contains("accept"));
        assert!(hdr.contains("content-type"));

        let methods = resp
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(methods.contains("POST"));
        assert!(methods.contains("GET"));
        assert!(methods.contains("OPTIONS"));

        Rc::get_mut(&mut cors.inner).unwrap().preflight = false;

        let req = TestRequest::with_header("Origin", "https://www.example.com")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                "AUTHORIZATION,ACCEPT",
            )
            .method(Method::OPTIONS)
            .to_srv_request();

        let resp = test::call_service(&mut cors, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
