use std::fmt;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::de;

use crate::{AsResult, Error, Parse};

/// The server keep-alive preference.
///
/// By default keep alive is set to 5 seconds. Takes a string value: Either "default", "disabled",
/// "os", or a string of the format "N seconds" where N is an integer > 0 e.g. "6 seconds".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeepAlive {
    /// The default keep-alive as defined by Actix Web.
    Default,

    /// Disable keep-alive.
    Disabled,

    /// Let the OS determine keep-alive duration.
    ///
    /// Note: this is usually quite long.
    Os,

    /// A specific keep-alive duration (in seconds).
    Seconds(usize),
}

impl Parse for KeepAlive {
    fn parse(string: &str) -> AsResult<Self> {
        pub(crate) static FMT: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\d+ seconds$").expect("Failed to compile regex: FMT"));

        pub(crate) static DIGITS: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\d+").expect("Failed to compile regex: FMT"));

        macro_rules! invalid_value {
            ($got:expr) => {
                Err(InvalidValue! {
                    expected: "a string of the format \"N seconds\" where N is an integer > 0",
                    got: $got,
                })
            };
        }

        let digits_in = |m: regex::Match<'_>| &string[m.start()..m.end()];
        match string {
            "default" => Ok(KeepAlive::Default),
            "disabled" => Ok(KeepAlive::Disabled),
            "OS" | "os" => Ok(KeepAlive::Os),
            string if !FMT.is_match(string) => invalid_value!(string),
            string => match DIGITS.find(string) {
                None => invalid_value!(string),
                Some(mat) => match digits_in(mat).parse() {
                    Ok(val) => Ok(KeepAlive::Seconds(val)),
                    Err(_) => invalid_value!(string),
                },
            },
        }
    }
}

impl<'de> de::Deserialize<'de> for KeepAlive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct KeepAliveVisitor;

        impl<'de> de::Visitor<'de> for KeepAliveVisitor {
            type Value = KeepAlive;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let msg = "Either \"default\", \"disabled\", \"os\", or a string of the format \"N seconds\" where N is an integer > 0";
                f.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match KeepAlive::parse(value) {
                    Ok(keep_alive) => Ok(keep_alive),
                    Err(Error::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(KeepAliveVisitor)
    }
}
