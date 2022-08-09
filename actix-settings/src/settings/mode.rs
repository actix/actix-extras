use serde::Deserialize;

use crate::{AsResult, Parse};

/// Marker of intended deployment environment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Marks development environment.
    Development,

    /// Marks production environment.
    Production,
}

impl Parse for Mode {
    fn parse(string: &str) -> AsResult<Self> {
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
