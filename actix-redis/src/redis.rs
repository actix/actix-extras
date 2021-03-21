use std::collections::VecDeque;
use std::net::SocketAddr;

use redis_async::client::{paired_connect, PairedConnection};
use redis_async::resp::RespValue;
use tokio::sync::Mutex;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use trust_dns_resolver::TokioAsyncResolver as AsyncResolver;

use crate::Error;

pub struct RedisClient {
    addr: String,
    connection: Mutex<Option<PairedConnection>>,
}

impl RedisClient {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            connection: Mutex::new(None),
        }
    }

    async fn get_connection(&self) -> Result<PairedConnection, Error> {
        let mut connection = self.connection.lock().await;
        if let Some(ref connection) = *connection {
            return Ok(connection.clone());
        }

        let mut addrs = resolve(&self.addr).await?;
        loop {
            // try to connect
            let socket_addr = addrs.pop_front().ok_or_else(|| {
                log::warn!("Cannot connect to {}.", self.addr);
                Error::NotConnected
            })?;
            match paired_connect(socket_addr).await {
                Ok(conn) => {
                    *connection = Some(conn.clone());
                    return Ok(conn);
                }
                Err(err) => log::warn!(
                    "Attempt to connect to {} as {} failed: {}.",
                    self.addr,
                    socket_addr,
                    err
                ),
            }
        }
    }

    pub async fn send(&self, req: RespValue) -> Result<RespValue, Error> {
        let res = self.get_connection().await?.send(req).await?;
        Ok(res)
    }
}

fn parse_addr(addr: &str, default_port: u16) -> Option<(&str, u16)> {
    // split the string by ':' and convert the second part to u16
    let mut parts_iter = addr.splitn(2, ':');
    let host = parts_iter.next()?;
    let port_str = parts_iter.next().unwrap_or("");
    let port: u16 = port_str.parse().unwrap_or(default_port);
    Some((host, port))
}

async fn resolve(addr: &str) -> Result<VecDeque<SocketAddr>, Error> {
    // try to parse as a regular SocketAddr first
    if let Ok(addr) = addr.parse::<SocketAddr>() {
        let mut addrs = VecDeque::new();
        addrs.push_back(addr);
        return Ok(addrs);
    }

    let (host, port) = parse_addr(addr, 6379).ok_or(Error::InvalidAddress)?;

    // we need to do dns resolution
    let resolver = AsyncResolver::tokio_from_system_conf()
        .or_else(|err| {
            log::warn!("Cannot create system DNS resolver: {}", err);
            AsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
        })
        .map_err(|err| {
            log::error!("Cannot create DNS resolver: {}", err);
            Error::ResolveError
        })?;

    let addrs = resolver
        .lookup_ip(host)
        .await
        .map_err(|_| Error::ResolveError)?
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .collect();

    Ok(addrs)
}
