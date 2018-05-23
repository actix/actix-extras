use base64;
use actix_web::FromRequest;
use actix_web::test::TestRequest;

use super::BasicAuth;

#[test]
fn test_valid_auth() {
    let value = format!("Basic {}", base64::encode("user:pass"));
    let req = TestRequest::with_header("Authorization", value).finish();
    let auth = BasicAuth::extract(&req);

    assert!(auth.is_ok());
    let auth = auth.unwrap();
    assert_eq!(auth.username, "user".to_string());
    assert_eq!(auth.password, "pass".to_string());
}

#[test]
fn test_missing_header() {
    let req = TestRequest::default().finish();
    let auth = BasicAuth::extract(&req);

    assert!(auth.is_err());
}

#[test]
fn test_invalid_mechanism() {
    let value = format!("Digest {}", base64::encode("user:pass"));
    let req = TestRequest::with_header("Authorization", value).finish();
    let auth = BasicAuth::extract(&req);

    assert!(auth.is_err());
}

#[test]
fn test_invalid_format() {
    let value = format!("Basic {}", base64::encode("user"));
    let req = TestRequest::with_header("Authorization", value).finish();
    let auth = BasicAuth::extract(&req);

    assert!(auth.is_err());
}

#[test]
fn test_user_without_password() {
    let value = format!("Basic {}", base64::encode("user:"));
    let req = TestRequest::with_header("Authorization", value).finish();
    let auth = BasicAuth::extract(&req);

    assert!(auth.is_ok());
    assert_eq!(auth.unwrap(), BasicAuth {
        username: "user".to_string(),
        password: "".to_string(),
    })
}
