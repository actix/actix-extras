/// A library to process Server.toml files
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
use serde::{de, Deserialize};

#[macro_use]
mod error;
mod actix;
mod core;

pub use self::actix::*;
pub use self::core::Parse;
pub use self::error::{AtError, AtResult};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(bound = "A: Deserialize<'de>")]
pub struct BasicSettings<A> {
    pub actix: ActixSettings,
    pub application: A,
}

pub type Settings = BasicSettings<NoSettings>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[non_exhaustive]
pub struct NoSettings {/* NOTE: turning this into a unit struct will cause deserialization failures. */}

impl<A> BasicSettings<A>
where
    A: de::DeserializeOwned,
{
    /// NOTE **DO NOT** mess with the ordering of the tables in this template.
    ///      Especially the `[application]` table needs to be last in order
    ///      for some tests to keep working.
    pub(crate) const DEFAULT_TOML_TEMPLATE: &'static str = include_str!("./defaults.toml");

    /// Parse an instance of `Self` from a `TOML` file located at `filepath`.
    /// If the file doesn't exist, it is generated from the default `TOML`
    /// template, after which the newly generated file is read in and parsed.
    pub fn parse_toml<P>(filepath: P) -> AtResult<Self>
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref();

        if !filepath.exists() {
            Self::write_toml_file(filepath)?;
        }

        let mut f = File::open(filepath)?;
        let mut contents = String::with_capacity(f.metadata()?.len() as usize);
        f.read_to_string(&mut contents)?;

        Ok(toml::from_str::<Self>(&contents)?)
    }

    /// Parse an instance of `Self` straight from the default `TOML` template.
    pub fn from_default_template() -> AtResult<Self> {
        Self::from_template(Self::DEFAULT_TOML_TEMPLATE)
    }

    /// Parse an instance of `Self` straight from the default `TOML` template.
    pub fn from_template(template: &str) -> AtResult<Self> {
        Ok(toml::from_str(template)?)
    }

    /// Write the default `TOML` template to a new file, to be located
    /// at `filepath`.  Return a `Error::FileExists(_)` error if a
    /// file already exists at that location.
    pub fn write_toml_file<P>(filepath: P) -> AtResult<()>
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref();
        let contents = Self::DEFAULT_TOML_TEMPLATE.trim();

        if filepath.exists() {
            return Err(AtError::FileExists(filepath.to_path_buf()));
        }

        let mut file = File::create(filepath)?;
        file.write_all(contents.as_bytes())?;
        file.flush()?;

        Ok(())
    }

    pub fn override_field<F, V>(field: &mut F, value: V) -> AtResult<()>
    where
        F: Parse,
        V: AsRef<str>,
    {
        *field = F::parse(value.as_ref())?;
        Ok(())
    }

    pub fn override_field_with_env_var<F, N>(field: &mut F, var_name: N) -> AtResult<()>
    where
        F: Parse,
        N: AsRef<str>,
    {
        match env::var(var_name.as_ref()) {
            Err(env::VarError::NotPresent) => Ok((/*NOP*/)),
            Err(var_error) => Err(AtError::from(var_error)),
            Ok(value) => Self::override_field(field, value),
        }
    }
}

pub trait ApplySettings {
    /// Apply a [`BasicSettings`] value to `self`.
    ///
    /// [`BasicSettings`]: ./struct.BasicSettings.html
    #[must_use]
    fn apply_settings<A>(self, settings: &BasicSettings<A>) -> Self
    where
        A: de::DeserializeOwned;
}

impl<F, I, S, B> ApplySettings for HttpServer<F, I, S, B>
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
    fn apply_settings<A>(mut self, settings: &BasicSettings<A>) -> Self
    where
        A: de::DeserializeOwned,
    {
        if settings.actix.tls.enabled {
            // for Address { host, port } in &settings.actix.hosts {
            //     self = self.bind(format!("{}:{}", host, port))
            //         .unwrap(/*TODO*/);
            // }
            todo!("[ApplySettings] TLS support has not been implemented yet.");
        } else {
            for Address { host, port } in &settings.actix.hosts {
                self = self.bind(format!("{}:{}", host, port))
                    .unwrap(/*TODO*/);
            }
        }

        self = match settings.actix.num_workers {
            NumWorkers::Default => self,
            NumWorkers::Manual(n) => self.workers(n),
        };

        self = match settings.actix.backlog {
            Backlog::Default => self,
            Backlog::Manual(n) => self.backlog(n as u32),
        };

        self = match settings.actix.max_connections {
            MaxConnections::Default => self,
            MaxConnections::Manual(n) => self.max_connections(n),
        };

        self = match settings.actix.max_connection_rate {
            MaxConnectionRate::Default => self,
            MaxConnectionRate::Manual(n) => self.max_connection_rate(n),
        };

        self = match settings.actix.keep_alive {
            KeepAlive::Default => self,
            KeepAlive::Disabled => self.keep_alive(ActixKeepAlive::Disabled),
            KeepAlive::Os => self.keep_alive(ActixKeepAlive::Os),
            KeepAlive::Seconds(n) => self.keep_alive(Duration::from_secs(n as u64)),
        };

        self = match settings.actix.client_timeout {
            Timeout::Default => self,
            Timeout::Milliseconds(n) => {
                self.client_disconnect_timeout(Duration::from_millis(n as u64))
            }
            Timeout::Seconds(n) => self.client_disconnect_timeout(Duration::from_secs(n as u64)),
        };

        self = match settings.actix.client_shutdown {
            Timeout::Default => self,
            Timeout::Milliseconds(n) => {
                self.client_disconnect_timeout(Duration::from_millis(n as u64))
            }
            Timeout::Seconds(n) => self.client_disconnect_timeout(Duration::from_secs(n as u64)),
        };

        self = match settings.actix.shutdown_timeout {
            Timeout::Default => self,
            Timeout::Milliseconds(_) => self.shutdown_timeout(1),
            Timeout::Seconds(n) => self.shutdown_timeout(n as u64),
        };

        self
    }
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use std::path::Path;

    use actix_web::{App, HttpServer};
    use serde::Deserialize;

    use crate::actix::*; // used for value construction in assertions
    use crate::{ApplySettings, AtResult, BasicSettings, Settings};

    #[test]
    fn apply_settings() -> AtResult<()> {
        let settings = Settings::parse_toml("Server.toml")?;
        let _ = HttpServer::new(App::new).apply_settings(&settings);
        Ok(())
    }

    #[test]
    fn override_field__hosts() {
        let mut settings = Settings::from_default_template().unwrap();

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
    fn override_field_with_env_var__hosts() {
        let mut settings = Settings::from_default_template().unwrap();

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
    fn override_field__mode() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.mode, Mode::Development);
        Settings::override_field(&mut settings.actix.mode, "production")?;
        assert_eq!(settings.actix.mode, Mode::Production);
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__mode() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.mode, Mode::Development);
        std::env::set_var("OVERRIDE__MODE", "production");
        Settings::override_field_with_env_var(&mut settings.actix.mode, "OVERRIDE__MODE")?;
        assert_eq!(settings.actix.mode, Mode::Production);
        Ok(())
    }

    #[test]
    fn override_field__enable_compression() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert!(settings.actix.enable_compression);
        Settings::override_field(&mut settings.actix.enable_compression, "false")?;
        assert!(!settings.actix.enable_compression);
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__enable_compression() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert!(settings.actix.enable_compression);
        std::env::set_var("OVERRIDE__ENABLE_COMPRESSION", "false");
        Settings::override_field_with_env_var(
            &mut settings.actix.enable_compression,
            "OVERRIDE__ENABLE_COMPRESSION",
        )?;
        assert!(!settings.actix.enable_compression);
        Ok(())
    }

    #[test]
    fn override_field__enable_log() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert!(settings.actix.enable_log);
        Settings::override_field(&mut settings.actix.enable_log, "false")?;
        assert!(!settings.actix.enable_log);
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__enable_log() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert!(settings.actix.enable_log);
        std::env::set_var("OVERRIDE__ENABLE_LOG", "false");
        Settings::override_field_with_env_var(
            &mut settings.actix.enable_log,
            "OVERRIDE__ENABLE_LOG",
        )?;
        assert!(!settings.actix.enable_log);
        Ok(())
    }

    #[test]
    fn override_field__num_workers() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.num_workers, NumWorkers::Default);
        Settings::override_field(&mut settings.actix.num_workers, "42")?;
        assert_eq!(settings.actix.num_workers, NumWorkers::Manual(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__num_workers() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.num_workers, NumWorkers::Default);
        std::env::set_var("OVERRIDE__NUM_WORKERS", "42");
        Settings::override_field_with_env_var(
            &mut settings.actix.num_workers,
            "OVERRIDE__NUM_WORKERS",
        )?;
        assert_eq!(settings.actix.num_workers, NumWorkers::Manual(42));
        Ok(())
    }

    #[test]
    fn override_field__backlog() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.backlog, Backlog::Default);
        Settings::override_field(&mut settings.actix.backlog, "42")?;
        assert_eq!(settings.actix.backlog, Backlog::Manual(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__backlog() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.backlog, Backlog::Default);
        std::env::set_var("OVERRIDE__BACKLOG", "42");
        Settings::override_field_with_env_var(&mut settings.actix.backlog, "OVERRIDE__BACKLOG")?;
        assert_eq!(settings.actix.backlog, Backlog::Manual(42));
        Ok(())
    }

    #[test]
    fn override_field__max_connections() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.max_connections, MaxConnections::Default);
        Settings::override_field(&mut settings.actix.max_connections, "42")?;
        assert_eq!(settings.actix.max_connections, MaxConnections::Manual(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__max_connections() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.max_connections, MaxConnections::Default);
        std::env::set_var("OVERRIDE__MAX_CONNECTIONS", "42");
        Settings::override_field_with_env_var(
            &mut settings.actix.max_connections,
            "OVERRIDE__MAX_CONNECTIONS",
        )?;
        assert_eq!(settings.actix.max_connections, MaxConnections::Manual(42));
        Ok(())
    }

    #[test]
    fn override_field__max_connection_rate() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Default
        );
        Settings::override_field(&mut settings.actix.max_connection_rate, "42")?;
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Manual(42)
        );
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__max_connection_rate() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Default
        );
        std::env::set_var("OVERRIDE__MAX_CONNECTION_RATE", "42");
        Settings::override_field_with_env_var(
            &mut settings.actix.max_connection_rate,
            "OVERRIDE__MAX_CONNECTION_RATE",
        )?;
        assert_eq!(
            settings.actix.max_connection_rate,
            MaxConnectionRate::Manual(42)
        );
        Ok(())
    }

    #[test]
    fn override_field__keep_alive() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.keep_alive, KeepAlive::Default);
        Settings::override_field(&mut settings.actix.keep_alive, "42 seconds")?;
        assert_eq!(settings.actix.keep_alive, KeepAlive::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__keep_alive() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.keep_alive, KeepAlive::Default);
        std::env::set_var("OVERRIDE__KEEP_ALIVE", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.keep_alive,
            "OVERRIDE__KEEP_ALIVE",
        )?;
        assert_eq!(settings.actix.keep_alive, KeepAlive::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field__client_timeout() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.client_timeout, Timeout::Default);
        Settings::override_field(&mut settings.actix.client_timeout, "42 seconds")?;
        assert_eq!(settings.actix.client_timeout, Timeout::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__client_timeout() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.client_timeout, Timeout::Default);
        std::env::set_var("OVERRIDE__CLIENT_TIMEOUT", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.client_timeout,
            "OVERRIDE__CLIENT_TIMEOUT",
        )?;
        assert_eq!(settings.actix.client_timeout, Timeout::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field__client_shutdown() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.client_shutdown, Timeout::Default);
        Settings::override_field(&mut settings.actix.client_shutdown, "42 seconds")?;
        assert_eq!(settings.actix.client_shutdown, Timeout::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__client_shutdown() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.client_shutdown, Timeout::Default);
        std::env::set_var("OVERRIDE__CLIENT_SHUTDOWN", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.client_shutdown,
            "OVERRIDE__CLIENT_SHUTDOWN",
        )?;
        assert_eq!(settings.actix.client_shutdown, Timeout::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field__shutdown_timeout() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Default);
        Settings::override_field(&mut settings.actix.shutdown_timeout, "42 seconds")?;
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__shutdown_timeout() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Default);
        std::env::set_var("OVERRIDE__SHUTDOWN_TIMEOUT", "42 seconds");
        Settings::override_field_with_env_var(
            &mut settings.actix.shutdown_timeout,
            "OVERRIDE__SHUTDOWN_TIMEOUT",
        )?;
        assert_eq!(settings.actix.shutdown_timeout, Timeout::Seconds(42));
        Ok(())
    }

    #[test]
    fn override_field__tls__enabled() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert!(!settings.actix.tls.enabled);
        Settings::override_field(&mut settings.actix.tls.enabled, "true")?;
        assert!(settings.actix.tls.enabled);
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__tls__enabled() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert!(!settings.actix.tls.enabled);
        std::env::set_var("OVERRIDE__TLS_ENABLED", "true");
        Settings::override_field_with_env_var(
            &mut settings.actix.tls.enabled,
            "OVERRIDE__TLS_ENABLED",
        )?;
        assert!(settings.actix.tls.enabled);
        Ok(())
    }

    #[test]
    fn override_field__tls__certificate() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("path/to/cert/cert.pem")
        );
        Settings::override_field(
            &mut settings.actix.tls.certificate,
            "/overridden/path/to/cert/cert.pem",
        )?;
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("/overridden/path/to/cert/cert.pem")
        );
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__tls__certificate() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
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
        )?;
        assert_eq!(
            settings.actix.tls.certificate,
            Path::new("/overridden/path/to/cert/cert.pem")
        );
        Ok(())
    }

    #[test]
    fn override_field__tls__private_key() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("path/to/cert/key.pem")
        );
        Settings::override_field(
            &mut settings.actix.tls.private_key,
            "/overridden/path/to/cert/key.pem",
        )?;
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("/overridden/path/to/cert/key.pem")
        );
        Ok(())
    }

    #[test]
    fn override_field_with_env_var__tls__private_key() -> AtResult<()> {
        let mut settings = Settings::from_default_template()?;
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
        )?;
        assert_eq!(
            settings.actix.tls.private_key,
            Path::new("/overridden/path/to/cert/key.pem")
        );
        Ok(())
    }

    #[test]
    fn override_extended_field_with_custom_type() -> AtResult<()> {
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
        struct NestedSetting {
            foo: String,
            bar: bool,
        }
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
        struct AppSettings {
            #[serde(rename = "example-name")]
            example_name: String,
            #[serde(rename = "nested-field")]
            nested_field: NestedSetting,
        }
        type CustomSettings = BasicSettings<AppSettings>;
        let mut settings = CustomSettings::from_template(
            &(CustomSettings::DEFAULT_TOML_TEMPLATE.to_string()
                // NOTE: Add these entries to the `[application]` table:
                + "\nexample-name = \"example value\""
                + "\nnested-field = { foo = \"foo\", bar = false }"),
        )?;
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
        )?;
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
        Ok(())
    }
}
