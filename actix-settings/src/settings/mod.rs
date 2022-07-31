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
    pub hosts: Vec<Address>,
    pub mode: mode::Mode,
    pub enable_compression: bool,
    pub enable_log: bool,
    pub num_workers: NumWorkers,
    pub backlog: Backlog,
    pub max_connections: MaxConnections,
    pub max_connection_rate: MaxConnectionRate,
    pub keep_alive: KeepAlive,
    pub client_timeout: Timeout,
    pub client_shutdown: Timeout,
    pub shutdown_timeout: Timeout,
    pub tls: Tls,
}
