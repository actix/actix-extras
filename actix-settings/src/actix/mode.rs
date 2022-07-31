use serde::Deserialize;

use crate::{core::Parse, error::AtError};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum Mode {
    #[serde(rename = "development")]
    Development,

    #[serde(rename = "production")]
    Production,
}

impl Parse for Mode {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        match string {
            "development" => Ok(Self::Development),
            "production" => Ok(Self::Production),
            _ => Err(InvalidValue! {
                expected: "\"development\" | \"production\".",
                got: string,
            }),
        }
    }
}
