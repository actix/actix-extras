#![cfg(feature = "memory-store")]

use std::{thread::sleep, time::Duration};

use actix_limitation::{Error, Limiter, MemoryStore, RateLimiter};
use actix_web::{
    dev::ServiceRequest, http::StatusCode, test as actix_test, web, App, HttpRequest, HttpResponse,
};

const LONG_PERIOD: Duration = Duration::from_secs(60);

const SHORT_PERIOD: Duration = Duration::from_secs(2);

const SHORT_PERIOD_PLUS: Duration = Duration::from_millis(2_500);

fn memory_limiter(limit: usize, period: Duration) -> Limiter {
    Limiter::memory_builder(MemoryStore::new())
        .limit(limit)
        .period(period)
        .build()
        .unwrap()
}

#[actix_web::test]
async fn test_limiter_count() -> Result<(), Error> {
    let limit = 20;
    let limiter = memory_limiter(limit, LONG_PERIOD);

    for i in 0..limit {
        let status = limiter.count("key").await?;
        assert_eq!(status.limit(), limit);
        assert_eq!(limit - status.remaining(), i + 1);
    }

    Ok(())
}

#[actix_web::test]
async fn test_limiter_count_error() -> Result<(), Error> {
    let limit = 5;
    let limiter = memory_limiter(limit, LONG_PERIOD);

    for i in 0..limit {
        let status = limiter.count("key").await?;
        assert_eq!(limit - status.remaining(), i + 1);
    }

    for _ in 0..3 {
        match limiter.count("key").await.unwrap_err() {
            Error::LimitExceeded(status) => {
                assert_eq!(status.limit(), limit);
                assert_eq!(status.remaining(), 0);
            }
            err => panic!("error should be LimitExceeded variant, got {err:?}"),
        }
    }

    let status = limiter.count("other-key").await?;
    assert_eq!(status.remaining(), limit - 1);

    Ok(())
}

#[actix_web::test]
async fn test_limiters_share_one_store() -> Result<(), Error> {
    let limit = 4;
    let store = MemoryStore::new();
    let first = Limiter::memory_builder(store.clone())
        .limit(limit)
        .period(LONG_PERIOD)
        .build()
        .unwrap();
    let second = Limiter::memory_builder(store)
        .limit(limit)
        .period(LONG_PERIOD)
        .build()
        .unwrap();

    assert_eq!(first.count("shared").await?.remaining(), 3);
    assert_eq!(second.count("shared").await?.remaining(), 2);

    Ok(())
}

#[actix_web::test]
async fn test_middleware_limits_requests() {
    let limit = 2;
    let limiter = Limiter::memory_builder(MemoryStore::new())
        .limit(limit)
        .period(LONG_PERIOD)
        .key_by(|_: &ServiceRequest| Some("fixed_key".to_owned()))
        .build()
        .unwrap();

    let app = actix_test::init_service(
        App::new()
            .wrap(RateLimiter::default())
            .app_data(web::Data::new(limiter))
            .route(
                "/",
                web::get().to(|_: HttpRequest| async { HttpResponse::Ok().body("ok") }),
            ),
    )
    .await;

    for index in 1..=5 {
        let req = actix_test::TestRequest::default().to_request();
        let resp = actix_test::call_service(&app, req).await;

        if index <= limit {
            assert!(
                resp.status().is_success(),
                "request {index}: {:?}",
                resp.status()
            );
        } else {
            assert_eq!(
                resp.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "request {index}"
            );
        }
    }
}

#[actix_web::test]
async fn test_middleware_passes_through_unkeyed_requests() {
    let app = actix_test::init_service(
        App::new()
            .wrap(RateLimiter::default())
            .app_data(web::Data::new(
                Limiter::memory_builder(MemoryStore::new())
                    .limit(1)
                    .period(LONG_PERIOD)
                    .key_by(|_: &ServiceRequest| None)
                    .build()
                    .unwrap(),
            ))
            .route(
                "/",
                web::get().to(|_: HttpRequest| async { HttpResponse::Ok().body("ok") }),
            ),
    )
    .await;

    for _ in 0..5 {
        let req = actix_test::TestRequest::default().to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}

#[actix_web::test]
async fn test_middleware_window_resets() {
    let limiter = Limiter::memory_builder(MemoryStore::new())
        .limit(1)
        .period(SHORT_PERIOD)
        .key_by(|_: &ServiceRequest| Some("fixed_key".to_owned()))
        .build()
        .unwrap();

    let app = actix_test::init_service(
        App::new()
            .wrap(RateLimiter::default())
            .app_data(web::Data::new(limiter))
            .route(
                "/",
                web::get().to(|_: HttpRequest| async { HttpResponse::Ok().body("ok") }),
            ),
    )
    .await;

    let call = || actix_test::call_service(&app, actix_test::TestRequest::default().to_request());

    assert!(call().await.status().is_success());
    assert_eq!(call().await.status(), StatusCode::TOO_MANY_REQUESTS);

    sleep(SHORT_PERIOD_PLUS);

    assert!(
        call().await.status().is_success(),
        "window did not reset for the middleware",
    );
}
