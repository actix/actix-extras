use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Tls {
    pub enabled: bool,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}
