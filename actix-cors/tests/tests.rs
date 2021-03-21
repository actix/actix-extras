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

fn val_as_str(val: &HeaderValue) -> &str {
    val.to_str().unwrap()
}

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
    let cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .allowed_origin_fn(|origin, req| {
            assert_eq!(&origin, req.headers.get(header::ORIGIN).unwrap());

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
        let req = TestRequest::get()
            .insert_header(("Origin", "https://www.example.com"))
            .to_srv_request();

        let resp = test::call_service(&cors, req).await;

        assert_eq!(
            Some(&b"https://www.example.com"[..]),
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(HeaderValue::as_bytes)
        );
    }

    {
        let req = TestRequest::get()
            .insert_header(("Origin", "https://www.known.com"))
            .to_srv_request();

        let resp = test::call_service(&cors, req).await;

        assert_eq!(
            None,
            resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );
    }
}

#[actix_rt::test]
async fn test_allowed_origin_fn() {
    let cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .allowed_origin_fn(|origin, req| {
            assert_eq!(&origin, req.headers.get(header::ORIGIN).unwrap());

            req.headers
                .get(header::ORIGIN)
                .map(HeaderValue::as_bytes)
                .filter(|b| b.ends_with(b".unknown.com"))
                .is_some()
        })
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::get()
        .insert_header(("Origin", "https://www.example.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;

    assert_eq!(
        "https://www.example.com",
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(val_as_str)
            .unwrap()
    );

    let req = TestRequest::get()
        .insert_header(("Origin", "https://www.unknown.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;

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

    let cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .allowed_origin_fn(move |origin, req| {
            assert_eq!(&origin, req.headers.get(header::ORIGIN).unwrap());

            req.headers
                .get(header::ORIGIN)
                .map(HeaderValue::as_bytes)
                .filter(|b| regex.is_match(b))
                .is_some()
        })
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::get()
        .insert_header(("Origin", "https://www.example.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;

    assert_eq!(
        "https://www.example.com",
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(val_as_str)
            .unwrap()
    );

    let req = TestRequest::get()
        .insert_header(("Origin", "https://www.unknown.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;

    assert_eq!(
        Some(&b"https://www.unknown.com"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
}

#[actix_rt::test]
async fn test_multiple_origins_preflight() {
    let cors = Cors::default()
        .allowed_origin("https://example.com")
        .allowed_origin("https://example.org")
        .allowed_methods(vec![Method::GET])
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::default()
        .insert_header(("Origin", "https://example.com"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
        .method(Method::OPTIONS)
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"https://example.com"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );

    let req = TestRequest::default()
        .insert_header(("Origin", "https://example.org"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
        .method(Method::OPTIONS)
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"https://example.org"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
}

#[actix_rt::test]
async fn test_multiple_origins() {
    let cors = Cors::default()
        .allowed_origin("https://example.com")
        .allowed_origin("https://example.org")
        .allowed_methods(vec![Method::GET])
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::get()
        .insert_header(("Origin", "https://example.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"https://example.com"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );

    let req = TestRequest::get()
        .insert_header(("Origin", "https://example.org"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"https://example.org"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
}

#[actix_rt::test]
async fn test_response() {
    let exposed_headers = vec![header::AUTHORIZATION, header::ACCEPT];
    let cors = Cors::default()
        .allow_any_origin()
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

    let req = TestRequest::default()
        .insert_header(("Origin", "https://www.example.com"))
        .method(Method::OPTIONS)
        .to_srv_request();
    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"*"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
    assert_eq!(
        Some(&b"Origin"[..]),
        resp.headers().get(header::VARY).map(HeaderValue::as_bytes)
    );

    #[allow(clippy::needless_collect)]
    {
        let headers = resp
            .headers()
            .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
            .map(val_as_str)
            .unwrap()
            .split(',')
            .map(|s| s.trim())
            .collect::<Vec<&str>>();

        // TODO: use HashSet subset check
        for h in exposed_headers {
            assert!(headers.contains(&h.as_str()));
        }
    }

    let exposed_headers = vec![header::AUTHORIZATION, header::ACCEPT];
    let cors = Cors::default()
        .allow_any_origin()
        .send_wildcard()
        .disable_preflight()
        .max_age(3600)
        .allowed_methods(vec![Method::GET, Method::OPTIONS, Method::POST])
        .allowed_headers(exposed_headers.clone())
        .expose_headers(exposed_headers.clone())
        .allowed_header(header::CONTENT_TYPE)
        .new_transform(fn_service(|req: ServiceRequest| {
            ok(req.into_response({
                HttpResponse::Ok()
                    .insert_header((header::VARY, "Accept"))
                    .finish()
            }))
        }))
        .await
        .unwrap();

    let req = TestRequest::default()
        .insert_header(("Origin", "https://www.example.com"))
        .method(Method::OPTIONS)
        .to_srv_request();
    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"Accept, Origin"[..]),
        resp.headers().get(header::VARY).map(HeaderValue::as_bytes)
    );

    let cors = Cors::default()
        .disable_vary_header()
        .allowed_methods(vec!["POST"])
        .allowed_origin("https://www.example.com")
        .allowed_origin("https://www.google.com")
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::default()
        .insert_header(("Origin", "https://www.example.com"))
        .method(Method::OPTIONS)
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .to_srv_request();
    let resp = test::call_service(&cors, req).await;
    let origins_str = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .map(val_as_str);
    assert_eq!(Some("https://www.example.com"), origins_str);
}

#[actix_rt::test]
async fn test_validate_origin() {
    let cors = Cors::default()
        .allowed_origin("https://www.example.com")
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::get()
        .insert_header(("Origin", "https://www.example.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_rt::test]
async fn test_no_origin_response() {
    let cors = Cors::permissive()
        .disable_preflight()
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::default().method(Method::GET).to_srv_request();
    let resp = test::call_service(&cors, req).await;
    assert!(resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());

    let req = TestRequest::default()
        .insert_header(("Origin", "https://www.example.com"))
        .method(Method::OPTIONS)
        .to_srv_request();
    let resp = test::call_service(&cors, req).await;
    assert_eq!(
        Some(&b"https://www.example.com"[..]),
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(HeaderValue::as_bytes)
    );
}

#[actix_rt::test]
async fn validate_origin_allows_all_origins() {
    let cors = Cors::permissive()
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::default()
        .insert_header(("Origin", "https://www.example.com"))
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_rt::test]
async fn test_allow_any_origin_any_method_any_header() {
    let cors = Cors::default()
        .allow_any_origin()
        .allow_any_method()
        .allow_any_header()
        .new_transform(test::ok_service())
        .await
        .unwrap();

    let req = TestRequest::default()
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type"))
        .insert_header((header::ORIGIN, "https://www.example.com"))
        .method(Method::OPTIONS)
        .to_srv_request();

    let resp = test::call_service(&cors, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
