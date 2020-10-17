use actix_service::fn_service;
use actix_web::{
    dev::{ServiceRequest, Transform},
    http::{header, HeaderValue, Method, StatusCode},
    test::{self, TestRequest},
    HttpResponse,
};
use futures_util::future::ok;
use regex::bytes::Regex;

use actix_cors::Cors;

#[actix_rt::test]
#[should_panic]
async fn test_wildcard_origin() {
    Cors::default()
        .allowed_origin("*")
        .new_transform(test::ok_service())
        .await
        .unwrap();
}

#[actix_rt::test]
async fn test_not_allowed_origin_fn() {
    let mut cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .allowed_origin_fn(|req| {
            req.headers
                .get(header::ORIGIN)
                .map(HeaderValue::as_bytes)
                .filter(|b| b.ends_with(b".unknown.com"))
                .is_some()
        })
        .new_transform(test::ok_service())
        .await
        .unwrap();

    {
        let req = TestRequest::with_header("Origin", "https://www.example.com")
            .method(Method::GET)
            .to_srv_request();

        let resp = test::call_service(&mut cors, req).await;

        assert_eq!(
            Some(&b"https://www.example.com"[..]),
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(HeaderValue::as_bytes)
        );
    }

    {
        let req = TestRequest::with_header("Origin", "https://www.known.com")
            .method(Method::GET)
            .to_srv_request();

        let resp = test::call_service(&mut cors, req).await;

        assert_eq!(
            None,
            resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );
    }
}

#[actix_rt::test]
async fn test_allowed_origin_fn() {
    let mut cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .allowed_origin_fn(|req| {
            req.headers
                .get(header::ORIGIN)
                .map(HeaderValue::as_bytes)
                .filter(|b| b.ends_with(b".unknown.com"))
                .is_some()
        })
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://www.example.com")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;

    assert_eq!(
        "https://www.example.com",
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .to_str()
            .unwrap()
    );

    let req = TestRequest::with_header("Origin", "https://www.unknown.com")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;

    assert_eq!(
        Some(&b"https://www.unknown.com"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
}

#[actix_rt::test]
async fn test_allowed_origin_fn_with_environment() {
    let regex = Regex::new("https:.+\\.unknown\\.com").unwrap();
    let mut cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .allowed_origin_fn(move |req| {
            req.headers
                .get(header::ORIGIN)
                .map(HeaderValue::as_bytes)
                .filter(|b| regex.is_match(b))
                .is_some()
        })
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://www.example.com")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;

    assert_eq!(
        "https://www.example.com",
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .to_str()
            .unwrap()
    );

    let req = TestRequest::with_header("Origin", "https://www.unknown.com")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;

    assert_eq!(
        Some(&b"https://www.unknown.com"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
}

#[actix_rt::test]
async fn test_multiple_origins_preflight() {
    let mut cors = Cors::default()
        .allowed_origin("https://example.com")
        .allowed_origin("https://example.org")
        .allowed_methods(vec![Method::GET])
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .method(Method::OPTIONS)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(
        &b"https://example.com"[..],
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .as_bytes()
    );

    let req = TestRequest::with_header("Origin", "https://example.org")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .method(Method::OPTIONS)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(
        &b"https://example.org"[..],
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .as_bytes()
    );
}

#[actix_rt::test]
async fn test_multiple_origins() {
    let mut cors = Cors::default()
        .allowed_origin("https://example.com")
        .allowed_origin("https://example.org")
        .allowed_methods(vec![Method::GET])
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://example.com")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(
        &b"https://example.com"[..],
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .as_bytes()
    );

    let req = TestRequest::with_header("Origin", "https://example.org")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(
        &b"https://example.org"[..],
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .as_bytes()
    );
}

#[actix_rt::test]
async fn test_response() {
    let exposed_headers = vec![header::AUTHORIZATION, header::ACCEPT];
    let mut cors = Cors::default()
        .send_wildcard()
        .disable_preflight()
        .max_age(3600)
        .allowed_methods(vec![Method::GET, Method::OPTIONS, Method::POST])
        .allowed_headers(exposed_headers.clone())
        .expose_headers(exposed_headers.clone())
        .allowed_header(header::CONTENT_TYPE)
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://www.example.com")
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
        &b"Origin"[..],
        resp.headers().get(header::VARY).unwrap().as_bytes()
    );

    #[allow(clippy::needless_collect)]
    {
        let headers = resp
            .headers()
            .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
            .unwrap()
            .to_str()
            .unwrap()
            .split(',')
            .map(|s| s.trim())
            .collect::<Vec<&str>>();

        for h in exposed_headers {
            assert!(headers.contains(&h.as_str()));
        }
    }

    let exposed_headers = vec![header::AUTHORIZATION, header::ACCEPT];
    let mut cors = Cors::default()
        .send_wildcard()
        .disable_preflight()
        .max_age(3600)
        .allowed_methods(vec![Method::GET, Method::OPTIONS, Method::POST])
        .allowed_headers(exposed_headers.clone())
        .expose_headers(exposed_headers.clone())
        .allowed_header(header::CONTENT_TYPE)
        .new_transform(fn_service(|req: ServiceRequest| {
            ok(req.into_response(
                HttpResponse::Ok().header(header::VARY, "Accept").finish(),
            ))
        }))
        .await
        .unwrap();
    let req = TestRequest::with_header("Origin", "https://www.example.com")
        .method(Method::OPTIONS)
        .to_srv_request();
    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(
        &b"Accept, Origin"[..],
        resp.headers().get(header::VARY).unwrap().as_bytes()
    );

    let mut cors = Cors::default()
        .disable_vary_header()
        .allowed_origin("https://www.example.com")
        .allowed_origin("https://www.google.com")
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://www.example.com")
        .method(Method::OPTIONS)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .to_srv_request();
    let resp = test::call_service(&mut cors, req).await;

    let origins_str = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .unwrap()
        .to_str()
        .unwrap();

    assert_eq!("https://www.example.com", origins_str);
}

#[actix_rt::test]
async fn test_validate_origin() {
    let mut cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::with_header("Origin", "https://www.example.com")
        .method(Method::GET)
        .to_srv_request();

    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_rt::test]
async fn test_no_origin_response() {
    let mut cors = Cors::default()
        .disable_preflight()
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::default().method(Method::GET).to_srv_request();
    let resp = test::call_service(&mut cors, req).await;
    assert!(resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());

    let req = TestRequest::with_header("Origin", "https://www.example.com")
        .method(Method::OPTIONS)
        .to_srv_request();
    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(
        &b"https://www.example.com"[..],
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap()
            .as_bytes()
    );
}

#[actix_rt::test]
async fn validate_origin_allows_all_origins() {
    let mut cors = Cors::permissive()
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req =
        TestRequest::with_header("Origin", "https://www.example.com").to_srv_request();

    let resp = test::call_service(&mut cors, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
