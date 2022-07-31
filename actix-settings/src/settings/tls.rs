use std::path::PathBuf;

use serde::Deserialize;

/// TLS (HTTPS) configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[doc(alias = "ssl", alias = "https")]
pub struct Tls {
    /// Tru if accepting TLS connections should be enabled.
    pub enabled: bool,

    /// Path to certificate `.pem` file.
    pub certificate: PathBuf,

    /// Path to private key `.pem` file.
    pub private_key: PathBuf,
}
