use std::str;
use std::fmt;

use base64;
use bytes::{BufMut, BytesMut};
use actix_web::http::header::{HeaderValue, IntoHeaderValue, InvalidHeaderValueBytes};

use headers::authorization::Scheme;
use headers::authorization::errors::ParseError;

/// Credentials for `Basic` authentication scheme, defined in [RFC 7617](https://tools.ietf.org/html/rfc7617)
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Basic {
    pub username: String,
    pub password: Option<String>,
}

impl Scheme for Basic {
    fn parse(header: &HeaderValue) -> Result<Self, ParseError> {
        // "Basic *" length
        if header.len() < 7 {
            return Err(ParseError::Invalid);
        }

        let mut parts = header.to_str()?.splitn(2, ' ');
        match parts.next() {
            Some(scheme) if scheme == "Basic" => (),
            _ => return Err(ParseError::MissingScheme),
        }

        let decoded = base64::decode(parts.next().ok_or(ParseError::Invalid)?)?;
        let mut credentials = str::from_utf8(&decoded)?
            .splitn(2, ':');

        let username = credentials.next()
            .ok_or(ParseError::MissingField("username"))
            .map(|username| username.to_string())?;
        let password = credentials.next()
            .ok_or(ParseError::MissingField("password"))
            .map(|password| {
                if password.is_empty() {
                    None
                } else {
                    Some(password.to_string())
                }
            })?;

        Ok(Basic{
            username,
            password,
        })
    }
}

impl fmt::Debug for Basic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("Basic {}:******", self.username))
    }
}

impl fmt::Display for Basic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: Display password also
        f.write_fmt(format_args!("Basic {}:******", self.username))
    }
}

impl IntoHeaderValue for Basic {
    type Error = InvalidHeaderValueBytes;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        let mut credentials = BytesMut::with_capacity(
            self.username.len() + 1 + self.password.as_ref().map_or(0, |pwd| pwd.len())
        );
        credentials.put(&self.username);
        credentials.put_u8(b':');
        if let Some(ref password) = self.password {
            credentials.put(password);
        }

        // TODO: It would be nice not to allocate new `String`  here but write directly to `value`
        let encoded = base64::encode(&credentials);
        let mut value = BytesMut::with_capacity(6 + encoded.len());
        value.put("Basic ");
        value.put(&encoded);

        HeaderValue::from_shared(value.freeze())
    }
}

#[cfg(test)]
mod tests {
    use actix_web::http::header::{HeaderValue, IntoHeaderValue};
    use super::{Scheme, Basic};

    #[test]
    fn test_parse_header() {
        let value = HeaderValue::from_static("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.username, "Aladdin");
        assert_eq!(scheme.password, Some("open sesame".to_string()));
    }

    #[test]
    fn test_empty_password() {
        let value = HeaderValue::from_static("Basic QWxhZGRpbjo=");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.username, "Aladdin");
        assert_eq!(scheme.password, None);
    }

    #[test]
    fn test_empty_header() {
        let value = HeaderValue::from_static("");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_wrong_scheme() {
        let value = HeaderValue::from_static("THOUSHALLNOTPASS please?");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_missing_credentials() {
        let value = HeaderValue::from_static("Basic ");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_missing_credentials_colon() {
        let value = HeaderValue::from_static("Basic QWxsYWRpbg==");
        let scheme = Basic::parse(&value);

        assert!(scheme.is_err());
    }

    #[test]
    fn test_into_header_value() {
        let basic = Basic {
            username: "Aladdin".to_string(),
            password: Some("open sesame".to_string()),
        };

        let result = basic.try_into();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), HeaderValue::from_static("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="));
    }
}


#[cfg(all(test, feature = "nightly"))]
mod benches {
    use test::Bencher;

    use actix_web::http::header::{HeaderValue, IntoHeaderValue};

    use super::{Basic, Scheme};

    #[bench]
    fn bench_parsing(b: &mut Bencher) {
        let value = HeaderValue::from_static("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        b.iter(|| {
            Basic::parse(&value)
        });
    }

    #[bench]
    fn bench_serializing(b: &mut Bencher) {
        b.iter(|| {
            let basic = Basic {
                username: "Aladdin".to_string(),
                password: Some("open sesame".to_string()),
            };

            basic.try_into()
        })
    }
}
