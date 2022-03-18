use actix_limitation::{Error, Limiter};
use uuid::Uuid;

#[test]
#[should_panic = "Redis URL did not parse"]
fn test_create_limiter_error() {
    Limiter::build("127.0.0.1").finish().unwrap();
}

// TODO: figure out whats wrong with this test
#[ignore]
#[actix_web::test]
async fn test_limiter_count() -> Result<(), Error> {
    let builder = Limiter::build("redis://127.0.0.1:6379/2");
    let limiter = builder.finish().unwrap();
    let id = Uuid::new_v4();

    for i in 0..5000 {
        let status = limiter.count(id.to_string()).await?;
        assert_eq!(5000 - status.remaining(), i + 1);
    }

    Ok(())
}

// TODO: figure out whats wrong with this test
#[ignore]
#[actix_web::test]
async fn test_limiter_count_error() -> Result<(), Error> {
    let builder = Limiter::build("redis://127.0.0.1:6379/3");
    let limiter = builder.finish().unwrap();
    let id = Uuid::new_v4();

    for i in 0..5000 {
        let status = limiter.count(id.to_string()).await?;
        assert_eq!(5000 - status.remaining(), i + 1);
    }

    match limiter.count(id.to_string()).await.unwrap_err() {
        Error::LimitExceeded(status) => assert_eq!(status.remaining(), 0),
        _ => panic!("error should be LimitExceeded variant"),
    };

    let id = Uuid::new_v4();
    for i in 0..5000 {
        let status = limiter.count(id.to_string()).await?;
        assert_eq!(5000 - status.remaining(), i + 1);
    }

    Ok(())
}
