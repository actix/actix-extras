use super::*;

#[test]
fn test_create_status() {
    let status = Status {
        limit: 100,
        remaining: 0,
        reset_epoch_utc: 1000,
    };

    assert_eq!(status.limit(), 100);
    assert_eq!(status.remaining(), 0);
    assert_eq!(status.reset_epoch_utc(), 1000);
}

#[test]
fn test_build_status() {
    let count = 200;
    let limit = 100;
    let status = Status::build_status(count, limit, 2000);
    assert_eq!(status.limit(), limit);
    assert_eq!(status.remaining(), 0);
    assert_eq!(status.reset_epoch_utc(), 2000);
}

#[test]
fn test_build_status_limit() {
    let limit = 100;
    let status = Status::build_status(0, limit, 2000);
    assert_eq!(status.limit(), limit);
    assert_eq!(status.remaining(), limit);
    assert_eq!(status.reset_epoch_utc(), 2000);
}

#[test]
fn test_epoch_utc_plus_zero() {
    let duration = Duration::from_secs(0);
    let seconds = Status::epoch_utc_plus(duration).unwrap();
    assert!(seconds as u64 >= duration.as_secs());
}

#[test]
fn test_epoch_utc_plus() {
    let duration = Duration::from_secs(10);
    let seconds = Status::epoch_utc_plus(duration).unwrap();
    assert!(seconds as u64 >= duration.as_secs() + 10);
}

#[test]
#[should_panic = "Source duration value is out of range for the target type"]
fn test_epoch_utc_plus_overflow() {
    let duration = Duration::from_secs(10000000000000000000);
    match Status::epoch_utc_plus(duration).unwrap_err() {
        error => panic!("{}", error),
    };
}
