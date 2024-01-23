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
    /// Generates an [`SslAcceptorBuilder`] with its settings. It is often used for the following method
    /// [`actix_web::server::HttpServer::bind_openssl`].
    ///
    /// # Example
    /// ```no_run
    /// use actix_settings::{ApplySettings, Settings};
    /// use actix_web::{get, App, HttpServer, Responder};
    ///
    /// #[get("/")]
    /// async fn index() -> impl Responder {
    ///     "Hello."
    /// }
    ///
    /// #[actix_web::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let settings = Settings::from_default_template();
    ///
    ///     HttpServer::new(|| {
    ///         App::new()
    ///             .service(index)
    ///     })
    ///     .apply_settings(&settings)
    ///     .bind(("127.0.0.1", 8080))?
    ///     .bind_openssl(("127.0.0.1", 8081), settings.actix.tls.get_ssl_acceptor_builder())?
    ///     .run()
    ///     .await
    /// }
    /// ```
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
