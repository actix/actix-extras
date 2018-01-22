use std::io;
use std::net::SocketAddr;
use std::collections::VecDeque;
use std::time::Duration;

use actix::Arbiter;
use trust_dns_resolver::ResolverFuture;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use trust_dns_resolver::lookup_ip::LookupIpFuture;
use futures::{Async, Future, Poll};
use tokio_core::reactor::Timeout;
use tokio_core::net::{TcpStream, TcpStreamNew};


#[derive(Fail, Debug)]
pub enum TcpConnectorError {
    /// Failed to resolve the hostname
    #[fail(display = "Failed resolving hostname: {}", _0)]
    Dns(String),

    /// Address is invalid
    #[fail(display = "Invalid input: {}", _0)]
    InvalidInput(&'static str),

    /// Connecting took too long
    #[fail(display = "Timeout out while establishing connection")]
    Timeout,

    /// Connection io error
    #[fail(display = "{}", _0)]
    IoError(io::Error),
}

pub struct TcpConnector {
    lookup: Option<LookupIpFuture>,
    port: u16,
    ips: VecDeque<SocketAddr>,
    error: Option<TcpConnectorError>,
    timeout: Timeout,
    stream: Option<TcpStreamNew>,
}

impl TcpConnector {

    pub fn new<S: AsRef<str>>(addr: S) -> TcpConnector {
        TcpConnector::with_timeout(addr, Duration::from_secs(1))
    }

    pub fn with_timeout<S: AsRef<str>>(addr: S, timeout: Duration) -> TcpConnector {
        // try to parse as a regular SocketAddr first
        if let Ok(addr) = addr.as_ref().parse() {
            let mut ips = VecDeque::new();
            ips.push_back(addr);

            TcpConnector {
                lookup: None,
                port: 0,
                ips: ips,
                error: None,
                stream: None,
                timeout: Timeout::new(timeout, Arbiter::handle()).unwrap() }
        } else {
            match TcpConnector::parse(addr.as_ref()) {
                Ok((host, port)) => {
                    // we need to do dns resolution
                    let resolve = match ResolverFuture::from_system_conf(Arbiter::handle()) {
                        Ok(resolve) => resolve,
                        Err(err) => {
                            warn!("Can not create system dns resolver: {}", err);
                            ResolverFuture::new(
                                ResolverConfig::default(),
                                ResolverOpts::default(),
                                Arbiter::handle())
                        }
                    };

                    TcpConnector {
                        lookup: Some(resolve.lookup_ip(host)),
                        port: port,
                        ips: VecDeque::new(),
                        error: None,
                        stream: None,
                        timeout: Timeout::new(timeout, Arbiter::handle()).unwrap() }
                },
                Err(err) =>
                    TcpConnector {
                        lookup: None,
                        port: 0,
                        ips: VecDeque::new(),
                        error: Some(err),
                        stream: None,
                        timeout: Timeout::new(timeout, Arbiter::handle()).unwrap() },
            }
        }
    }

    fn parse(addr: &str) -> Result<(&str, u16), TcpConnectorError> {
        macro_rules! try_opt {
            ($e:expr, $msg:expr) => (
                match $e {
                    Some(r) => r,
                    None => return Err(TcpConnectorError::InvalidInput($msg)),
                }
            )
        }

        // split the string by ':' and convert the second part to u16
        let mut parts_iter = addr.rsplitn(2, ':');
        let port_str = try_opt!(parts_iter.next(), "invalid socket address");
        let host = try_opt!(parts_iter.next(), "invalid socket address");
        let port: u16 = try_opt!(port_str.parse().ok(), "invalid port value");

        Ok((host, port))
    }
}

impl Future for TcpConnector {
    type Item = TcpStream;
    type Error = TcpConnectorError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(err) = self.error.take() {
            Err(err)
        } else {
            // timeout
            if let Ok(Async::Ready(_)) = self.timeout.poll() {
                return Err(TcpConnectorError::Timeout)
            }

            // lookip ips
            if let Some(mut lookup) = self.lookup.take() {
                match lookup.poll() {
                    Ok(Async::NotReady) => {
                        self.lookup = Some(lookup);
                        return Ok(Async::NotReady)
                    },
                    Ok(Async::Ready(ips)) => {
                        let port = self.port;
                        let ips = ips.iter().map(|ip| SocketAddr::new(ip, port));
                        self.ips.extend(ips);
                        if self.ips.is_empty() {
                            return Err(TcpConnectorError::Dns(
                                "Expect at least one A dns record".to_owned()))
                        }
                    },
                    Err(err) => return Err(TcpConnectorError::Dns(format!("{}", err))),
                }
            }

            // connect
            loop {
                if let Some(mut new) = self.stream.take() {
                    match new.poll() {
                        Ok(Async::Ready(sock)) =>
                            return Ok(Async::Ready(sock)),
                        Ok(Async::NotReady) => {
                            self.stream = Some(new);
                            return Ok(Async::NotReady)
                        },
                        Err(err) => {
                            if self.ips.is_empty() {
                                return Err(TcpConnectorError::IoError(err))
                            }
                        }
                    }
                }

                // try to connect
                let addr = self.ips.pop_front().unwrap();
                self.stream = Some(TcpStream::connect(&addr, Arbiter::handle()));
            }
        }
    }
}
