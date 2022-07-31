use serde::Deserialize;

use crate::{AtResult, Parse};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Development,
    Production,
}

impl Parse for Mode {
    fn parse(string: &str) -> AtResult<Self> {
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
