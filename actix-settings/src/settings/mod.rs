use serde::Deserialize;

mod address;
mod backlog;
mod keep_alive;
mod max_connection_rate;
mod max_connections;
mod mode;
mod num_workers;
mod timeout;
mod tls;

pub use self::address::Address;
pub use self::backlog::Backlog;
pub use self::keep_alive::KeepAlive;
pub use self::max_connection_rate::MaxConnectionRate;
pub use self::max_connections::MaxConnections;
pub use self::mode::Mode;
pub use self::num_workers::NumWorkers;
pub use self::timeout::Timeout;
pub use self::tls::Tls;

/// Settings types for Actix Web.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ActixSettings {
    /// List of addresses for the server to bind to.
    pub hosts: Vec<Address>,

    /// Marker of intended deployment environment.
    pub mode: Mode,

    /// True if the [`Compress`](actix_web::middleware::Compress) middleware should be enabled.
    pub enable_compression: bool,

    /// True if the [`Logger`](actix_web::middleware::Logger) middleware should be enabled.
    pub enable_log: bool,

    /// The number of workers that the server should start.
    pub num_workers: NumWorkers,

    /// The maximum number of pending connections.
    pub backlog: Backlog,

    /// The per-worker maximum number of concurrent connections.
    pub max_connections: MaxConnections,

    /// The per-worker maximum concurrent TLS connection limit.
    pub max_connection_rate: MaxConnectionRate,

    /// Server keep-alive preference.
    pub keep_alive: KeepAlive,

    /// Timeout duration for reading client request header.
    pub client_timeout: Timeout,

    /// Timeout duration for connection shutdown.
    pub client_shutdown: Timeout,

    /// Timeout duration for graceful worker shutdown.
    pub shutdown_timeout: Timeout,

    /// TLS (HTTPS) configuration.
    pub tls: Tls,
}
