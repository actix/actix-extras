use actix_limitation::{Error, Limiter};
use uuid::Uuid;

#[test]
#[should_panic = "Redis URL did not parse"]
fn test_create_limiter_error() {
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
