use std::fmt;

use serde::de;

use crate::{AsResult, Error, Parse};

/// The maximum per-worker number of concurrent connections.
///
/// All socket listeners will stop accepting connections when this limit is reached for each worker.
/// By default max connections is set to a 25k. Takes a string value: Either "default", or an
/// integer N > 0 e.g. "6".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaxConnections {
    /// The default number of connections. See struct docs.
    Default,

    /// A specific number of connections.
    Manual(usize),
}

impl Parse for MaxConnections {
    fn parse(string: &str) -> AsResult<Self> {
        match string {
            "default" => Ok(MaxConnections::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(MaxConnections::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "an integer > 0",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> de::Deserialize<'de> for MaxConnections {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct MaxConnectionsVisitor;

        impl<'de> de::Visitor<'de> for MaxConnectionsVisitor {
            type Value = MaxConnections;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                f.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match MaxConnections::parse(value) {
                    Ok(max_connections) => Ok(max_connections),
                    Err(Error::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(MaxConnectionsVisitor)
    }
}
