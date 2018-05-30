use std::str;
use std::fmt;
use std::default::Default;

use bytes::{BufMut, Bytes, BytesMut};
use actix_web::http::header::{HeaderValue, IntoHeaderValue, InvalidHeaderValueBytes};

use super::Challenge;

/// Challenge for `WWW-Authenticate` header with HTTP Basic auth scheme,
/// described in [RFC 7617](https://tools.ietf.org/html/rfc7617)
#[derive(Debug, Clone)]
pub struct Basic {
    // "realm" parameter is optional now: https://tools.ietf.org/html/rfc7235#appendix-A
    pub realm: Option<String>,
}

impl Challenge for Basic {
    fn to_bytes(&self) -> Bytes {
        // 5 is for `"Basic"`, 9 is for `"realm=\"\""`
        let length = 5 + self.realm.as_ref().map_or(0, |realm| realm.len() + 9);
        let mut buffer = BytesMut::with_capacity(length);
        buffer.put("Basic");
        if let Some(ref realm) = self.realm {
            buffer.put(" realm=\"");
            buffer.put(realm);
            buffer.put("\"");
        }

        buffer.freeze()
    }
}

impl fmt::Display for Basic {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let bytes = self.to_bytes();
        let repr = str::from_utf8(&bytes)
            // Should not happen since challenges are crafted manually
            // from `&'static str`'s and Strings
            .map_err(|_| fmt::Error)?;

        f.write_str(repr)
    }
}

impl IntoHeaderValue for Basic {
    type Error = InvalidHeaderValueBytes;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        HeaderValue::from_shared(self.to_bytes())
    }
}


impl Default for Basic {
    fn default() -> Self {
        Self {
            realm: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Basic;
    use actix_web::http::header::IntoHeaderValue;

    #[test]
    fn test_plain_into_header_value() {
        let challenge = Basic {
            realm: None,
        };

        let value = challenge.try_into();
        assert!(value.is_ok());
        let value = value.unwrap();
        assert_eq!(value, "Basic");
    }

    #[test]
    fn test_with_realm_into_header_value() {
        let challenge = Basic {
            realm: Some("Restricted area".to_string()),
        };

        let value = challenge.try_into();
        assert!(value.is_ok());
        let value = value.unwrap();
        assert_eq!(value, "Basic realm=\"Restricted area\"");
    }
}
