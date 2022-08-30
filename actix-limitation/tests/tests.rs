use std::time::Duration;

use actix_limitation::{Error, Limiter, RateLimiter};
use actix_web::{dev::ServiceRequest, http::StatusCode, test, web, App, HttpRequest, HttpResponse};
use uuid::Uuid;

#[test]
#[should_panic = "Redis URL did not parse"]
async fn test_create_limiter_error() {
    Limiter::builder("127.0.0.1").build().unwrap();
}

#[actix_web::test]
async fn test_limiter_count() -> Result<(), Error> {
    let limiter = Limiter::builder("redis://127.0.0.1:6379/2")
        .limit(20)
        .build()
        .unwrap();

    let id = Uuid::new_v4();

    for i in 0..20 {
        let status = limiter.count(id.to_string()).await?;
        println!("status: {:?}", status);
        assert_eq!(20 - status.remaining(), i + 1);
    }

    Ok(())
}

#[actix_web::test]
async fn test_limiter_count_error() -> Result<(), Error> {
    let limiter = Limiter::builder("redis://127.0.0.1:6379/3")
        .limit(25)
        .build()
        .unwrap();

    let id = Uuid::new_v4();
    for i in 0..25 {
        let status = limiter.count(id.to_string()).await?;
        assert_eq!(25 - status.remaining(), i + 1);
    }

    match limiter.count(id.to_string()).await.unwrap_err() {
        Error::LimitExceeded(status) => assert_eq!(status.remaining(), 0),
        _ => panic!("error should be LimitExceeded variant"),
    };

    let id = Uuid::new_v4();
    for i in 0..25 {
        let status = limiter.count(id.to_string()).await?;
        assert_eq!(25 - status.remaining(), i + 1);
    }

    Ok(())
}

#[actix_web::test]
async fn test_limiter_get_key() -> Result<(), Error> {
    let cooldown_period = Duration::from_secs(1);
    let limiter = Limiter::builder("redis://127.0.0.1:6379/3")
        .limit(2)
        .period(cooldown_period)
        .get_key(|_: &ServiceRequest| Some("fix_key".to_string()))
        .build()
        .unwrap();

    async fn index(_req: HttpRequest) -> HttpResponse {
        HttpResponse::Ok().body("ok")
    }
    let app = test::init_service(
        App::new()
            .wrap(RateLimiter::default())
            .app_data(web::Data::new(limiter))
            .route("/", web::get().to(index)),
    )
    .await;
    for _ in 1..2 {
        for index in 1..4 {
            let req = test::TestRequest::default().to_request();
            let resp = test::call_service(&app, req).await;
            if index <= 2 {
                assert!(resp.status().is_success());
            } else {
                assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
            }
        }
        std::thread::sleep(cooldown_period);
    }

    Ok(())
}
