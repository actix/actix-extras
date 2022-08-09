use std::fmt;

use serde::de;

use crate::{AsResult, Error, Parse};

/// The maximum number of pending connections.
///
/// This refers to the number of clients that can be waiting to be served. Exceeding this number
/// results in the client getting an error when attempting to connect. It should only affect servers
/// under significant load.
///
/// Generally set in the 64–2048 range. The default value is 2048. Takes a string value: Either
/// "default", or an integer N > 0 e.g. "6".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Backlog {
    /// The default number of connections. See struct docs.
    Default,

    /// A specific number of connections.
    Manual(usize),
}

impl Parse for Backlog {
    fn parse(string: &str) -> AsResult<Self> {
        match string {
            "default" => Ok(Backlog::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(Backlog::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "an integer > 0",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> de::Deserialize<'de> for Backlog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct BacklogVisitor;

        impl<'de> de::Visitor<'de> for BacklogVisitor {
            type Value = Backlog;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                f.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Backlog::parse(value) {
                    Ok(backlog) => Ok(backlog),
                    Err(Error::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(BacklogVisitor)
    }
}
