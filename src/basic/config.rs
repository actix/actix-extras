use std::default::Default;

use bytes::Bytes;
use percent_encoding;
use actix_web::http::header::{HeaderValue, IntoHeaderValue, InvalidHeaderValue};

/// Challenge configuration for [BasicAuth](./struct.BasicAuth.html) extractor.
#[derive(Debug, Clone)]
pub struct Config {
    // "realm" parameter is optional now: https://tools.ietf.org/html/rfc7235#appendix-A
    realm: Option<String>,
}

impl Config {
    pub fn realm(&mut self, value: String) -> &mut Self {
        self.realm = Some(value);
        self
    }

    fn as_bytes(&self) -> Bytes {
        let mut bytes = Bytes::from_static(b"Basic");
        if let Some(ref realm) = self.realm {
            bytes.extend_from_slice(b" realm=\"");
            let realm = percent_encoding::utf8_percent_encode(realm, percent_encoding::SIMPLE_ENCODE_SET);
            for part in realm {
                bytes.extend_from_slice(part.as_bytes());
            }
            bytes.extend_from_slice(b"\"");
        }

        bytes
    }
}

impl IntoHeaderValue for Config {
    type Error = InvalidHeaderValue;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        HeaderValue::from_bytes(&self.as_bytes())
    }
}

impl<'a> IntoHeaderValue for &'a Config {
    type Error = InvalidHeaderValue;

    fn try_into(self) -> Result<HeaderValue, <Self as IntoHeaderValue>::Error> {
        HeaderValue::from_bytes(&self.as_bytes())
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            realm: None,
        }
    }
}
