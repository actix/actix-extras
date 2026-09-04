#![cfg(feature = "memory-store")]

use std::{thread::sleep, time::Duration};

use actix_limitation::{Error, Limiter, MemoryStore, MemoryStoreBuilder, RateLimiter};
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

#[test]
fn memory_store_traits() {
    static_assertions::assert_impl_all!(MemoryStore: Clone, Default, Send, Sync, std::fmt::Debug);
    static_assertions::assert_impl_all!(MemoryStoreBuilder: Send, Sync, std::fmt::Debug);
}

#[test]
fn memory_store_debug_hides_counters() {
    let store = MemoryStore::builder()
        .max_keys(64)
        .sweep_interval(Duration::from_secs(5))
        .build();

    let repr = format!("{store:?}");

    assert!(repr.starts_with("MemoryStore"), "{repr}");
    assert!(repr.contains("counters"), "{repr}");
}

#[test]
fn limiter_from_memory_builder_is_infallible() {
    assert!(Limiter::memory_builder(MemoryStore::new()).build().is_ok());
    assert!(Limiter::memory_builder(MemoryStore::default())
        .build()
        .is_ok());
    assert!(Limiter::memory_builder(
        MemoryStore::builder()
            .max_keys(10)
            .sweep_interval(Duration::from_secs(1))
            .build()
    )
    .build()
    .is_ok());
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
async fn test_distinct_keys_do_not_interfere() -> Result<(), Error> {
    let limit = 3;
    let limiter = memory_limiter(limit, LONG_PERIOD);

    assert_eq!(limiter.count("a").await?.remaining(), 2);
    assert_eq!(limiter.count("a").await?.remaining(), 1);
    assert_eq!(limiter.count("b").await?.remaining(), 2);
    assert_eq!(limiter.count("a").await?.remaining(), 0);
    assert_eq!(limiter.count("b").await?.remaining(), 1);

    assert!(matches!(
        limiter.count("a").await.unwrap_err(),
        Error::LimitExceeded(_),
    ));
    assert_eq!(limiter.count("b").await?.remaining(), 0);

    Ok(())
}

#[actix_web::test]
async fn test_clones_share_counters() -> Result<(), Error> {
    let limit = 4;

    let limiter = memory_limiter(limit, LONG_PERIOD);
    let cloned_limiter = limiter.clone();

    assert_eq!(limiter.count("key").await?.remaining(), 3);
    assert_eq!(cloned_limiter.count("key").await?.remaining(), 2);
    assert_eq!(limiter.count("key").await?.remaining(), 1);

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
async fn test_window_resets_after_period() -> Result<(), Error> {
    let limit = 5;
    let limiter = memory_limiter(limit, SHORT_PERIOD);

    assert_eq!(limiter.count("key").await?.remaining(), limit - 1);
    assert_eq!(limiter.count("key").await?.remaining(), limit - 2);

    sleep(SHORT_PERIOD_PLUS);

    assert_eq!(
        limiter.count("key").await?.remaining(),
        limit - 1,
        "counter did not reset after the window elapsed",
    );

    Ok(())
}

#[actix_web::test]
async fn test_window_does_not_slide() -> Result<(), Error> {
    let gap = Duration::from_secs(3);
    let limit = 10;
    let limiter = memory_limiter(limit, LONG_PERIOD);

    let first = limiter.count("key").await?;

    sleep(gap);

    let second = limiter.count("key").await?;

    assert_eq!(
        second.remaining(),
        first.remaining() - 1,
        "expected the second request to land in the first request's window",
    );

    assert!(
        second.reset_epoch_utc() <= first.reset_epoch_utc() + 1,
        "window slid: reset moved from {} to {} after a {:?} gap",
        first.reset_epoch_utc(),
        second.reset_epoch_utc(),
        gap,
    );

    Ok(())
}

#[actix_web::test]
async fn test_max_keys_fails_open() -> Result<(), Error> {
    let limiter = Limiter::memory_builder(
        MemoryStore::builder()
            .max_keys(1)
            .sweep_interval(LONG_PERIOD)
            .build(),
    )
    .limit(1)
    .period(LONG_PERIOD)
    .build()
    .unwrap();

    assert_eq!(limiter.count("tracked").await?.remaining(), 0);
    assert!(matches!(
        limiter.count("tracked").await.unwrap_err(),
        Error::LimitExceeded(_),
    ));

    for key in ["overflow-a", "overflow-b"] {
        for _ in 0..5 {
            let status = limiter.count(key).await?;
            assert_eq!(status.remaining(), 0, "limit is 1, so one unit is consumed");
        }
    }

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
