use std::path::PathBuf;

use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
use serde::Deserialize;

/// TLS (HTTPS) configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[doc(alias = "ssl", alias = "https")]
pub struct Tls {
    /// True if accepting TLS connections should be enabled.
    pub enabled: bool,

    /// Path to certificate `.pem` file.
    pub certificate: PathBuf,

    /// Path to private key `.pem` file.
    pub private_key: PathBuf,
}

impl Tls {
    pub fn get_ssl_acceptor_builder(&self) -> SslAcceptorBuilder {
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        builder
            .set_certificate_chain_file(&self.certificate)
            .unwrap();
        builder
            .set_private_key_file(&self.private_key, SslFiletype::PEM)
            .unwrap();
        builder
    }
}
