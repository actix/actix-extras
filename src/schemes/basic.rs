use base64;
use actix_web::{HttpRequest, HttpMessage, FromRequest};

use errors::Error;

/// Extractor for `Authorization: Basic {payload}` HTTP request header.
///
/// If header is not present, HTTP 401 will be returned.
///
/// # Example
///
/// use actix_web_httpauth::BasicAuth;
///
/// pub fn handler(auth: BasicAuth) -> String {
///     format!("Hello, {}", auth.username)
/// }
#[derive(Debug, PartialEq)]
pub struct BasicAuth {
    pub username: String,
    pub password: String,
}


impl<S> FromRequest<S> for BasicAuth {
    type Config = ();
    type Result = Result<Self, Error>;

    fn from_request(req: &HttpRequest<S>, _cfg: &<Self as FromRequest<S>>::Config) -> <Self as FromRequest<S>>::Result {
        let header = req.headers().get("Authorization")
            .ok_or(Error::HeaderMissing)?
            .to_str()?;
        let mut parts = header.splitn(2, ' ');

        // Authorization mechanism
        match parts.next() {
            None => return Err(Error::InvalidMechanism),
            Some(mechanism) if mechanism != "Basic" => return Err(Error::InvalidMechanism),
            _ => ()
        }

        // Authorization payload
        let payload = parts.next().ok_or(Error::HeaderMalformed)?;
        let payload = base64::decode(payload)?;
        let payload = String::from_utf8(payload)?;
        let mut parts = payload.splitn(2, ':');
        let user = parts.next().ok_or(Error::HeaderMalformed)?;
        let password = parts.next().ok_or(Error::HeaderMalformed)?;

        Ok(BasicAuth{
            username: user.to_string(),
            password: password.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use base64;
    use actix_web::FromRequest;
    use actix_web::test::TestRequest;

    use super::{BasicAuth, Error};

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
        let err = auth.err().unwrap();
        assert_eq!(err, Error::HeaderMissing);
    }

    #[test]
    fn test_invalid_mechanism() {
        let value = format!("Digest {}", base64::encode("user:pass"));
        let req = TestRequest::with_header("Authorization", value).finish();
        let auth = BasicAuth::extract(&req);

        assert!(auth.is_err());
        let err = auth.err().unwrap();
        assert_eq!(err, Error::InvalidMechanism);
    }

    #[test]
    fn test_invalid_format() {
        let value = format!("Basic {}", base64::encode("user"));
        let req = TestRequest::with_header("Authorization", value).finish();
        let auth = BasicAuth::extract(&req);

        assert!(auth.is_err());
        let err = auth.err().unwrap();
        assert_eq!(err, Error::HeaderMalformed);
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
}
