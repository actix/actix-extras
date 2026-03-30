use std::path::PathBuf;

#[cfg(feature = "openssl")]
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
#[cfg(feature = "rustls-0_23")]
use rustls_0_23::{
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    ServerConfig as Rustls023ServerConfig,
};
use serde::Deserialize;

use crate::AsResult;

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
    /// Returns an [`SslAcceptorBuilder`] with the configured settings.
    ///
    /// The result is often used with [`actix_web::HttpServer::bind_openssl()`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::io;
    /// use actix_settings::{ApplySettings as _, Settings};
    /// use actix_web::{get, web, App, HttpServer, Responder};
    ///
    /// #[actix_web::main]
    /// async fn main() -> io::Result<()> {
    ///     let settings = Settings::from_default_template();
    ///
    ///     HttpServer::new(|| {
    ///         App::new().route("/", web::to(|| async { "Hello, World!" }))
    ///     })
    ///     .try_apply_settings(&settings)?
    ///     .bind(("127.0.0.1", 8080))?
    ///     .bind_openssl(("127.0.0.1", 8443), settings.actix.tls.get_ssl_acceptor_builder()?)?
    ///     .run()
    ///     .await
    /// }
    /// ```
    #[cfg(feature = "openssl")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openssl")))]
    pub fn get_ssl_acceptor_builder(&self) -> AsResult<SslAcceptorBuilder> {
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
        builder.set_certificate_chain_file(&self.certificate)?;
        builder.set_private_key_file(&self.private_key, SslFiletype::PEM)?;
        builder.check_private_key()?;

        Ok(builder)
    }

    /// Returns a [`Rustls023ServerConfig`] with the configured settings.
    ///
    /// The result is often used with [`actix_web::HttpServer::bind_rustls_0_23()`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::io;
    /// use actix_settings::{ApplySettings as _, Settings};
    /// use actix_web::{web, App, HttpServer};
    ///
    /// #[actix_web::main]
    /// async fn main() -> io::Result<()> {
    ///     let settings = Settings::from_default_template();
    ///
    ///     HttpServer::new(|| {
    ///         App::new().route("/", web::to(|| async { "Hello, World!" }))
    ///     })
    ///     .try_apply_settings(&settings)?
    ///     .bind_rustls_0_23(
    ///         ("127.0.0.1", 8443),
    ///         settings.actix.tls.get_rustls_0_23_server_config()?,
    ///     )?
    ///     .run()
    ///     .await
    /// }
    /// ```
    #[cfg(feature = "rustls-0_23")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-0_23")))]
    pub fn get_rustls_0_23_server_config(&self) -> AsResult<Rustls023ServerConfig> {
        let cert_chain = CertificateDer::pem_file_iter(&self.certificate)
            .map_err(|err| crate::Error::RustlsError(err.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| crate::Error::RustlsError(err.to_string()))?;
        let private_key = PrivateKeyDer::from_pem_file(&self.private_key)
            .map_err(|err| crate::Error::RustlsError(err.to_string()))?;

        Ok(Rustls023ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)?)
    }
}
