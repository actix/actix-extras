use std::fmt;

use serde::de;

use crate::{AsResult, Error, Parse};

/// The number of workers that the server should start.
///
/// By default the number of available logical cpu cores is used. Takes a string value: Either
/// "default", or an integer N > 0 e.g. "6".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumWorkers {
    /// The default number of workers. See struct docs.
    Default,

    /// A specific number of workers.
    Manual(usize),
}

impl Parse for NumWorkers {
    fn parse(string: &str) -> AsResult<Self> {
        match string {
            "default" => Ok(NumWorkers::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(NumWorkers::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "a positive integer",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> de::Deserialize<'de> for NumWorkers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct NumWorkersVisitor;

        impl<'de> de::Visitor<'de> for NumWorkersVisitor {
            type Value = NumWorkers;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                f.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match NumWorkers::parse(value) {
                    Ok(num_workers) => Ok(num_workers),
                    Err(Error::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(NumWorkersVisitor)
    }
}
