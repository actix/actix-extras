use std::fmt;

use serde::de;

use crate::{AtError, AtResult, Parse};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaxConnectionRate {
    Default,
    Manual(usize),
}

impl Parse for MaxConnectionRate {
    fn parse(string: &str) -> AtResult<Self> {
        match string {
            "default" => Ok(MaxConnectionRate::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(MaxConnectionRate::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "an integer > 0",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> de::Deserialize<'de> for MaxConnectionRate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct MaxConnectionRateVisitor;

        impl<'de> de::Visitor<'de> for MaxConnectionRateVisitor {
            type Value = MaxConnectionRate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match MaxConnectionRate::parse(value) {
                    Ok(max_connection_rate) => Ok(max_connection_rate),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(MaxConnectionRateVisitor)
    }
}
