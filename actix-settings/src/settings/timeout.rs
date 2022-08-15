use std::fmt;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::de;

use crate::{AsResult, Error, Parse};

/// A timeout duration in milliseconds or seconds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Timeout {
    /// The default timeout. Depends on context.
    Default,

    /// Timeout in milliseconds.
    Milliseconds(usize),

    /// Timeout in seconds.
    Seconds(usize),
}

impl Parse for Timeout {
    fn parse(string: &str) -> AsResult<Self> {
        pub static FMT: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"^\d+ (milliseconds|seconds)$").expect("Failed to compile regex: FMT")
        });

        pub static DIGITS: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\d+").expect("Failed to compile regex: DIGITS"));

        pub static UNIT: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(milliseconds|seconds)$").expect("Failed to compile regex: UNIT")
        });

        macro_rules! invalid_value {
            ($got:expr) => {
                Err(InvalidValue! {
                    expected: "a string of the format \"N seconds\" or \"N milliseconds\" where N is an integer > 0",
                    got: $got,
                })
            }
        }

        match string {
            "default" => Ok(Timeout::Default),

            string if !FMT.is_match(string) => invalid_value!(string),

            string => match (DIGITS.find(string), UNIT.find(string)) {
                (None, _) | (_, None) => invalid_value!(string),

                (Some(digits), Some(unit)) => {
                    let digits = &string[digits.range()];
                    let unit = &string[unit.range()];

                    match (digits.parse(), unit) {
                        (Ok(n), "milliseconds") => Ok(Timeout::Milliseconds(n)),
                        (Ok(n), "seconds") => Ok(Timeout::Seconds(n)),
                        _ => invalid_value!(string),
                    }
                }
            },
        }
    }
}

impl<'de> de::Deserialize<'de> for Timeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct TimeoutVisitor;

        impl<'de> de::Visitor<'de> for TimeoutVisitor {
            type Value = Timeout;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let msg = "Either \"default\", \"disabled\", \"os\", or a string of the format \"N seconds\" where N is an integer > 0";
                f.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Timeout::parse(value) {
                    Ok(num_workers) => Ok(num_workers),
                    Err(Error::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(TimeoutVisitor)
    }
}
