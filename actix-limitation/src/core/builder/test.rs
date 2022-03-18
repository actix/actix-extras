use super::*;

#[test]
fn test_create_builder() {
    let redis_url = "redis://127.0.0.1";
    let period = Duration::from_secs(10);
    let builder = Builder {
        redis_url,
        limit: 100,
        period,
        cookie_name: "session".to_string(),
        session_key: "rate-api".to_string(),
    };

    assert_eq!(builder.redis_url, redis_url);
    assert_eq!(builder.limit, 100);
    assert_eq!(builder.period, period);
    assert_eq!(builder.session_key, "rate-api");
    assert_eq!(builder.cookie_name, "session");
}

#[test]
fn test_create_limiter() {
    let redis_url = "redis://127.0.0.1";
    let period = Duration::from_secs(20);
    let mut builder = Builder {
        redis_url,
        limit: 100,
        period: Duration::from_secs(10),
        session_key: "key".to_string(),
        cookie_name: "sid".to_string(),
    };

    let limiter = builder
        .limit(200)
        .period(period)
        .cookie_name("session".to_string())
        .session_key("rate-api".to_string())
        .finish()
        .unwrap();

    assert_eq!(limiter.limit, 200);
    assert_eq!(limiter.period, period);
    assert_eq!(limiter.session_key, "rate-api");
    assert_eq!(limiter.cookie_name, "session");
}

#[test]
#[should_panic = "Redis URL did not parse"]
fn test_create_limiter_error() {
    let redis_url = "127.0.0.1";
    let period = Duration::from_secs(20);
    let mut builder = Builder {
        redis_url,
        limit: 100,
        period: Duration::from_secs(10),
        session_key: "key".to_string(),
        cookie_name: "sid".to_string(),
    };

    match builder.limit(200).period(period).finish().unwrap_err() {
        Error::Client(error) => panic!("{}", error),
        _ => (),
    };
}
