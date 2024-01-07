//! Easily manage Actix Web's settings from a TOML file and environment variables.
//!
//! To get started add a [`Settings::parse_toml("./Server.toml")`](Settings::parse_toml) call to the
//! top of your main function. This will create a template file with descriptions of all the
//! configurable settings. You can change or remove anything in that file and it will be picked up
//! the next time you run your application.
//!
//! Overriding parts of the file can be done from values using [`Settings::override_field`] or from
//! the environment using [`Settings::override_field_with_env_var`].
//!
//! # Examples
//!
//! See examples folder on GitHub for complete example.
//!
//! ```ignore
//! # use actix_web::{
//! #     get,
//! #     middleware::{Compress, Condition, Logger},
//! #     web, App, HttpServer,
//! # };
//! use actix_settings::{ApplySettings as _, Mode, Settings};
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let mut settings = Settings::parse_toml("./Server.toml")
//!         .expect("Failed to parse `Settings` from Server.toml");
//!
//!     // If the environment variable `$APPLICATION__HOSTS` is set,
//!     // have its value override the `settings.actix.hosts` setting:
//!     Settings::override_field_with_env_var(&mut settings.actix.hosts, "APPLICATION__HOSTS")?;
//!
//!     init_logger(&settings);
//!
//!     HttpServer::new({
//!         // clone settings into each worker thread
//!         let settings = settings.clone();
//!
//!         move || {
//!             App::new()
//!                 // Include this `.wrap()` call for compression settings to take effect
//!                 .wrap(Condition::new(
//!                     settings.actix.enable_compression,
//!                     Compress::default(),
//!                 ))
//!
//!                 // add request logger
//!                 .wrap(Logger::default())
//!
//!                 // make `Settings` available to handlers
//!                 .app_data(web::Data::new(settings.clone()))
//!
//!                 // add request handlers as normal
//!                 .service(index)
//!         }
//!     })
//!     // apply the `Settings` to Actix Web's `HttpServer`
//!     .apply_settings(&settings)
//!     .run()
//!     .await
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible, missing_docs, missing_debug_implementations)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use std::{
    env, fmt,
    fs::File,
    io::{Read as _, Write as _},
    path::Path,
    time::Duration,
};

use actix_http::{Request, Response};
use actix_service::IntoServiceFactory;
use actix_web::{
    body::MessageBody,
    dev::{AppConfig, ServiceFactory},
    http::KeepAlive as ActixKeepAlive,
    Error as WebError, HttpServer,
};
use openssl::ssl::{SslAcceptor, SslMethod};
use serde::{de, Deserialize};

#[macro_use]
mod error;
mod parse;
mod settings;

pub use self::{
    error::Error,
    parse::Parse,
    settings::{
        ActixSettings, Address, Backlog, KeepAlive, MaxConnectionRate, MaxConnections, Mode,
        NumWorkers, Timeout, Tls,
    },
};

/// Convenience type alias for `Result<T, AtError>`.
type AsResult<T> = std::result::Result<T, Error>;

/// Wrapper for server and application-specific settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(bound = "A: Deserialize<'de>")]
pub struct BasicSettings<A> {
    /// Actix Web server settings.
    pub actix: ActixSettings,

    /// Application-specific settings.
    pub application: A,
}

/// Convenience type alias for [`BasicSettings`] with no defined application-specific settings.
pub type Settings = BasicSettings<NoSettings>;

/// Marker type representing no defined application-specific settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[non_exhaustive]
pub struct NoSettings {/* NOTE: turning this into a unit struct will cause deserialization failures. */}

impl<A> BasicSettings<A>
where
    A: de::DeserializeOwned,
{
    // NOTE **DO NOT** mess with the ordering of the tables in the default template.
    //      Especially the `[application]` table needs to be last in order
    //      for some tests to keep working.
    /// Default settings file contents.
    pub(crate) const DEFAULT_TOML_TEMPLATE: &'static str = include_str!("./defaults.toml");

    /// Parse an instance of `Self` from a TOML file located at `filepath`.
    ///
    /// If the file doesn't exist, it is generated from the default TOML template, after which the
    /// newly generated file is read in and parsed.
    pub fn parse_toml<P>(filepath: P) -> AsResult<Self>
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref();

        if !filepath.exists() {
            Self::write_toml_file(filepath)?;
        }

        let mut f = File::open(filepath)?;
        let len_guess = f.metadata().map(|md| md.len()).unwrap_or(128);

        let mut contents = String::with_capacity(len_guess as usize);
        f.read_to_string(&mut contents)?;

        Ok(toml::from_str::<Self>(&contents)?)
    }

    /// Parse an instance of `Self` straight from the default TOML template.
    pub fn from_default_template() -> Self {
        Self::from_template(Self::DEFAULT_TOML_TEMPLATE).unwrap()
    }

    /// Parse an instance of `Self` straight from the default TOML template.
    pub fn from_template(template: &str) -> AsResult<Self> {
        Ok(toml::from_str(template)?)
    }

    /// Writes the default TOML template to a new file, located at `filepath`.
    ///
    /// # Errors
    ///
    /// Returns a [`FileExists`](crate::Error::FileExists) error if a file already exists at that
    /// location.
    pub fn write_toml_file<P>(filepath: P) -> AsResult<()>
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref();

        if filepath.exists() {
            return Err(Error::FileExists(filepath.to_path_buf()));
        }

        let mut file = File::create(filepath)?;
        file.write_all(Self::DEFAULT_TOML_TEMPLATE.trim().as_bytes())?;
        file.flush()?;

        Ok(())
    }

    /// Attempts to parse `value` and override the referenced `field`.
    ///
    /// # Examples
    /// ```
    /// use actix_settings::{Settings, Mode};
    ///
    /// # fn inner() -> Result<(), actix_settings::Error> {
    /// let mut settings = Settings::from_default_template();
    /// assert_eq!(settings.actix.mode, Mode::Development);
    ///
    /// Settings::override_field(&mut settings.actix.mode, "production")?;
    /// assert_eq!(settings.actix.mode, Mode::Production);
    /// # Ok(()) }
    /// ```
    pub fn override_field<F, V>(field: &mut F, value: V) -> AsResult<()>
    where
        F: Parse,
        V: AsRef<str>,
    {
        *field = F::parse(value.as_ref())?;
        Ok(())
    }

    /// Attempts to read an environment variable, parse it, and override the referenced `field`.
    ///
    /// # Examples
    /// ```
    /// use actix_settings::{Settings, Mode};
    ///
    /// std::env::set_var("OVERRIDE__MODE", "production");
    ///
    /// # fn inner() -> Result<(), actix_settings::Error> {
    /// let mut settings = Settings::from_default_template();
    /// assert_eq!(settings.actix.mode, Mode::Development);
    ///
    /// Settings::override_field_with_env_var(&mut settings.actix.mode, "OVERRIDE__MODE")?;
    /// assert_eq!(settings.actix.mode, Mode::Production);
    /// # Ok(()) }
    /// ```
    pub fn override_field_with_env_var<F, N>(field: &mut F, var_name: N) -> AsResult<()>
    where
        F: Parse,
        N: AsRef<str>,
    {
        match env::var(var_name.as_ref()) {
            Err(env::VarError::NotPresent) => Ok((/*NOP*/)),
            Err(var_error) => Err(Error::from(var_error)),
            Ok(value) => Self::override_field(field, value),
        }
    }
}

/// Extension trait for applying parsed settings to the server object.
pub trait ApplySettings<S> {
    /// Apply some settings object value to `self`.
    #[must_use]
    fn apply_settings(self, settings: &S) -> Self;
}

impl<F, I, S, B> ApplySettings<ActixSettings> for HttpServer<F, I, S, B>
where
    F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<S, Request>,
    S: ServiceFactory<Request, Config = AppConfig> + 'static,
    S::Error: Into<WebError> + 'static,
    S::InitError: fmt::Debug,
    S::Response: Into<Response<B>> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    fn apply_settings(mut self, settings: &ActixSettings) -> Self {
        for Address { host, port } in &settings.hosts {
            #[cfg(feature = "tls")]
            {
                if settings.tls.enabled {
                    self = self.bind_openssl(format!("{}:{}", host, port), settings.tls.get_ssl_acceptor_builder())
                        .unwrap(/*TODO*/)
                } else {
                    self = self.bind(format!("{host}:{port}"))
                        .unwrap(/*TODO*/);
                }
            }

            #[cfg(not(feature = "tls"))]
            {
                self = self.bind(format!("{host}:{port}"))
                    .unwrap(/*TODO*/);
            }
        }

        self = match settings.num_workers {
            NumWorkers::Default => self,
            NumWorkers::Manual(n) => self.workers(n),
        };

        self = match settings.backlog {
            Backlog::Default => self,
            Backlog::Manual(n) => self.backlog(n as u32),
        };

        self = match settings.max_connections {
            MaxConnections::Default => self,
            MaxConnections::Manual(n) => self.max_connections(n),
        };

        self = match settings.max_connection_rate {
            MaxConnectionRate::Default => self,
            MaxConnectionRate::Manual(n) => self.max_connection_rate(n),
        };

        self = match settings.keep_alive {
            KeepAlive::Default => self,
            KeepAlive::Disabled => self.keep_alive(ActixKeepAlive::Disabled),
            KeepAlive::Os => self.keep_alive(ActixKeepAlive::Os),
            KeepAlive::Seconds(n) => self.keep_alive(Duration::from_secs(n as u64)),
        };

        self = match settings.client_timeout {
            Timeout::Default => self,
            Timeout::Milliseconds(n) => {
                self.client_request_timeout(Duration::from_millis(n as u64))
            }
            Timeout::Seconds(n) => self.client_request_timeout(Duration::from_secs(n as u64)),
        };

        self = match settings.client_shutdown {
            Timeout::Default => self,
            Timeout::Milliseconds(n) => {
                self.client_disconnect_timeout(Duration::from_millis(n as u64))
            }
            Timeout::Seconds(n) => self.client_disconnect_timeout(Duration::from_secs(n as u64)),
        };

        self = match settings.shutdown_timeout {
            Timeout::Default => self,
            Timeout::Milliseconds(_) => self.shutdown_timeout(1),
            Timeout::Seconds(n) => self.shutdown_timeout(n as u64),
        };

        self
    }
}

impl<F, I, S, B, A> ApplySettings<BasicSettings<A>> for HttpServer<F, I, S, B>
where
    F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<S, Request>,
    S: ServiceFactory<Request, Config = AppConfig> + 'static,
    S::Error: Into<WebError> + 'static,
    S::InitError: fmt::Debug,
    S::Response: Into<Response<B>> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
    A: de::DeserializeOwned,
{
    fn apply_settings(self, settings: &BasicSettings<A>) -> Self {
        self.apply_settings(&settings.actix)
    }
}

#[cfg(test)]
mod tests {
    use actix_web::App;

    use super::*;

    #[test]
    fn apply_settings() {
        let settings = Settings::parse_toml("Server.toml").unwrap();
        let _ = HttpServer::new(App::new).apply_settings(&settings);
    }

    #[test]
    fn override_field_hosts() {
        let mut settings = Settings::from_default_template();

        assert_eq!(
            settings.actix.hosts,
            vec![Address {
                host: "0.0.0.0".into(),
                port: 9000
            },]
        );

        Settings::override_field(
            &mut settings.actix.hosts,
            r#"[
            ["0.0.0.0",   1234],
            ["localhost", 2345]
        ]"#,
        )
        .unwrap();

        assert_eq!(
            settings.actix.hosts,
            vec![
                Address {
                    host: "0.0.0.0".into(),
                    port: 1234
                },
                Address {
                    host: "localhost".into(),
                    port: 2345
                },
            ]
        );
    }

    #[test]
    fn override_field_with_env_var_hosts() {
        let mut settings = Settings::from_default_template();

        assert_eq!(
            settings.actix.hosts,
            vec![Address {
                host: "0.0.0.0".into(),
                port: 9000
            },]
        );

        std::env::set_var(
            "OVERRIDE__HOSTS",
            r#"[
            ["0.0.0.0",   1234],
            ["localhost", 2345]
        ]"#,
        );

        Settings::override_field_with_env_var(&mut settings.actix.hosts, "OVERRIDE__HOSTS")
            .unwrap();

        assert_eq!(
            settings.actix.hosts,
            vec![
                Address {
                    host: "0.0.0.0".into(),
                    port: 1234
                },
                Address {
                    host: "localhost".into(),
                    port: 2345
                },
            ]
        );
    }

    #[test]
    fn override_field_mode() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.mode, Mode::Development);
        Settings::override_field(&mut settings.actix.mode, "production").unwrap();
        assert_eq!(settings.actix.mode, Mode::Production);
    }

    #[test]
    fn override_field_with_env_var_mode() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.mode, Mode::Development);
        std::env::set_var("OVERRIDE__MODE", "production");
        Settings::override_field_with_env_var(&mut settings.actix.mode, "OVERRIDE__MODE").unwrap();
        assert_eq!(settings.actix.mode, Mode::Production);
    }

    #[test]
    fn override_field_enable_compression() {
        let mut settings = Settings::from_default_template();
        assert!(settings.actix.enable_compression);
        Settings::override_field(&mut settings.actix.enable_compression, "false").unwrap();
        assert!(!settings.actix.enable_compression);
    }

    #[test]
    fn override_field_with_env_var_enable_compression() {
        let mut settings = Settings::from_default_template();
        assert!(settings.actix.enable_compression);
        std::env::set_var("OVERRIDE__ENABLE_COMPRESSION", "false");
        Settings::override_field_with_env_var(
            &mut settings.actix.enable_compression,
            "OVERRIDE__ENABLE_COMPRESSION",
        )
        .unwrap();
        assert!(!settings.actix.enable_compression);
    }

    #[test]
    fn override_field_enable_log() {
        let mut settings = Settings::from_default_template();
        assert!(settings.actix.enable_log);
        Settings::override_field(&mut settings.actix.enable_log, "false").unwrap();
        assert!(!settings.actix.enable_log);
    }

    #[test]
    fn override_field_with_env_var_enable_log() {
        let mut settings = Settings::from_default_template();
        assert!(settings.actix.enable_log);
        std::env::set_var("OVERRIDE__ENABLE_LOG", "false");
        Settings::override_field_with_env_var(
            &mut settings.actix.enable_log,
            "OVERRIDE__ENABLE_LOG",
        )
        .unwrap();
        assert!(!settings.actix.enable_log);
    }

    #[test]
    fn override_field_num_workers() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.num_workers, NumWorkers::Default);
        Settings::override_field(&mut settings.actix.num_workers, "42").unwrap();
        assert_eq!(settings.actix.num_workers, NumWorkers::Manual(42));
    }

    #[test]
    fn override_field_with_env_var_num_workers() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.num_workers, NumWorkers::Default);
        std::env::set_var("OVERRIDE__NUM_WORKERS", "42");
        Settings::override_field_with_env_var(
            &mut settings.actix.num_workers,
            "OVERRIDE__NUM_WORKERS",
        )
        .unwrap();
        assert_eq!(settings.actix.num_workers, NumWorkers::Manual(42));
    }

    #[test]
    fn override_field_backlog() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.backlog, Backlog::Default);
        Settings::override_field(&mut settings.actix.backlog, "42").unwrap();
        assert_eq!(settings.actix.backlog, Backlog::Manual(42));
    }

    #[test]
    fn override_field_with_env_var_backlog() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.backlog, Backlog::Default);
        std::env::set_var("OVERRIDE__BACKLOG", "42");
        Settings::override_field_with_env_var(&mut settings.actix.backlog, "OVERRIDE__BACKLOG")
            .unwrap();
        assert_eq!(settings.actix.backlog, Backlog::Manual(42));
    }

    #[test]
    fn override_field_max_connections() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.max_connections, MaxConnections::Default);
        Settings::override_field(&mut settings.actix.max_connections, "42").unwrap();
        assert_eq!(settings.actix.max_connections, MaxConnections::Manual(42));
    }

    #[test]
    fn override_field_with_env_var_max_connections() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.max_connections, MaxConnections::Default);
        std::env::set_var("OVERRIDE__MAX_CONNECTIONS", "42");
        Settings::override_field_with_env_var(
            &mut settings.actix.max_connections,
            "OVERRIDE__MAX_CONNECTIONS",
        )
        .unwrap();
        assert_eq!(settings.actix.max_connections, MaxConnections::Manual(42));
    }

    #[test]
    fn override_field_max_connection_rate() {
        let mut settings = Settings::from_default_template();
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Default
        );
        Settings::override_field(&mut settings.actix.max_connection_rate, "42").unwrap();
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Manual(42)
        );
    }

    #[test]
    fn override_field_with_env_var_max_connection_rate() {
        let mut settings = Settings::from_default_template();
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Default
        );
        std::env::set_var("OVERRIDE__MAX_CONNECTION_RATE", "42");
        Settings::override_field_with_env_var(
            &mut settings.actix.max_connection_rate,
            "OVERRIDE__MAX_CONNECTION_RATE",
        )
        .unwrap();
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Manual(42)
        );
    }

    #[test]
    fn override_field_keep_alive() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.keep_alive, KeepAlive::Default);
        Settings::override_field(&mut settings.actix.keep_alive, "42 seconds").unwrap();
        assert_eq!(settings.actix.keep_alive, KeepAlive::Seconds(42));
    }

    #[test]
    fn override_field_with_env_var_keep_alive() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.keep_alive, KeepAlive::Default);
        std::env::set_var("OVERRIDE__KEEP_ALIVE", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.keep_alive,
            "OVERRIDE__KEEP_ALIVE",
        )
        .unwrap();
        assert_eq!(settings.actix.keep_alive, KeepAlive::Seconds(42));
    }

    #[test]
    fn override_field_client_timeout() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.client_timeout, Timeout::Default);
        Settings::override_field(&mut settings.actix.client_timeout, "42 seconds").unwrap();
        assert_eq!(settings.actix.client_timeout, Timeout::Seconds(42));
    }

    #[test]
    fn override_field_with_env_var_client_timeout() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.client_timeout, Timeout::Default);
        std::env::set_var("OVERRIDE__CLIENT_TIMEOUT", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.client_timeout,
            "OVERRIDE__CLIENT_TIMEOUT",
        )
        .unwrap();
        assert_eq!(settings.actix.client_timeout, Timeout::Seconds(42));
    }

    #[test]
    fn override_field_client_shutdown() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.client_shutdown, Timeout::Default);
        Settings::override_field(&mut settings.actix.client_shutdown, "42 seconds").unwrap();
        assert_eq!(settings.actix.client_shutdown, Timeout::Seconds(42));
    }

    #[test]
    fn override_field_with_env_var_client_shutdown() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.client_shutdown, Timeout::Default);
        std::env::set_var("OVERRIDE__CLIENT_SHUTDOWN", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.client_shutdown,
            "OVERRIDE__CLIENT_SHUTDOWN",
        )
        .unwrap();
        assert_eq!(settings.actix.client_shutdown, Timeout::Seconds(42));
    }

    #[test]
    fn override_field_shutdown_timeout() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Default);
        Settings::override_field(&mut settings.actix.shutdown_timeout, "42 seconds").unwrap();
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Seconds(42));
    }

    #[test]
    fn override_field_with_env_var_shutdown_timeout() {
        let mut settings = Settings::from_default_template();
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Default);
        std::env::set_var("OVERRIDE__SHUTDOWN_TIMEOUT", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.shutdown_timeout,
            "OVERRIDE__SHUTDOWN_TIMEOUT",
        )
        .unwrap();
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Seconds(42));
    }

    #[test]
    fn override_field_tls_enabled() {
        let mut settings = Settings::from_default_template();
        assert!(!settings.actix.tls.enabled);
        Settings::override_field(&mut settings.actix.tls.enabled, "true").unwrap();
        assert!(settings.actix.tls.enabled);
    }

    #[test]
    fn override_field_with_env_var_tls_enabled() {
        let mut settings = Settings::from_default_template();
        assert!(!settings.actix.tls.enabled);
        std::env::set_var("OVERRIDE__TLS_ENABLED", "true");
        Settings::override_field_with_env_var(
            &mut settings.actix.tls.enabled,
            "OVERRIDE__TLS_ENABLED",
        )
        .unwrap();
        assert!(settings.actix.tls.enabled);
    }

    #[test]
    fn override_field_tls_certificate() {
        let mut settings = Settings::from_default_template();
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("path/to/cert/cert.pem")
        );
        Settings::override_field(
            &mut settings.actix.tls.certificate,
            "/overridden/path/to/cert/cert.pem",
        )
        .unwrap();
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("/overridden/path/to/cert/cert.pem")
        );
    }

    #[test]
    fn override_field_with_env_var_tls_certificate() {
        let mut settings = Settings::from_default_template();
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("path/to/cert/cert.pem")
        );
        std::env::set_var(
            "OVERRIDE__TLS_CERTIFICATE",
            "/overridden/path/to/cert/cert.pem",
        );
        Settings::override_field_with_env_var(
            &mut settings.actix.tls.certificate,
            "OVERRIDE__TLS_CERTIFICATE",
        )
        .unwrap();
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("/overridden/path/to/cert/cert.pem")
        );
    }

    #[test]
    fn override_field_tls_private_key() {
        let mut settings = Settings::from_default_template();
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("path/to/cert/key.pem")
        );
        Settings::override_field(
            &mut settings.actix.tls.private_key,
            "/overridden/path/to/cert/key.pem",
        )
        .unwrap();
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("/overridden/path/to/cert/key.pem")
        );
    }

    #[test]
    fn override_field_with_env_var_tls_private_key() {
        let mut settings = Settings::from_default_template();
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("path/to/cert/key.pem")
        );
        std::env::set_var(
            "OVERRIDE__TLS_PRIVATE_KEY",
            "/overridden/path/to/cert/key.pem",
        );
        Settings::override_field_with_env_var(
            &mut settings.actix.tls.private_key,
            "OVERRIDE__TLS_PRIVATE_KEY",
        )
        .unwrap();
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("/overridden/path/to/cert/key.pem")
        );
    }

    #[test]
    fn override_extended_field_with_custom_type() {
        #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
        struct NestedSetting {
            foo: String,
            bar: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct AppSettings {
            example_name: String,
            nested_field: NestedSetting,
        }

        type CustomSettings = BasicSettings<AppSettings>;

        let mut settings = CustomSettings::from_template(
            &(CustomSettings::DEFAULT_TOML_TEMPLATE.to_string()
                // NOTE: Add these entries to the `[application]` table:
                + "\nexample-name = \"example value\""
                + "\nnested-field = { foo = \"foo\", bar = false }"),
        )
        .unwrap();

        assert_eq!(
            settings.application,
            AppSettings {
                example_name: "example value".into(),
                nested_field: NestedSetting {
                    foo: "foo".into(),
                    bar: false,
                },
            }
        );

        CustomSettings::override_field(
            &mut settings.application.example_name,
            "/overridden/path/to/cert/key.pem",
        )
        .unwrap();

        assert_eq!(
            settings.application,
            AppSettings {
                example_name: "/overridden/path/to/cert/key.pem".into(),
                nested_field: NestedSetting {
                    foo: "foo".into(),
                    bar: false,
                },
            }
        );
    }
}
