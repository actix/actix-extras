use std::fmt;

use serde::de;

use crate::{AtError, AtResult, Parse};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Backlog {
    Default,
    Manual(usize),
}

impl Parse for Backlog {
    fn parse(string: &str) -> AtResult<Self> {
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

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Backlog::parse(value) {
                    Ok(backlog) => Ok(backlog),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(BacklogVisitor)
    }
}
