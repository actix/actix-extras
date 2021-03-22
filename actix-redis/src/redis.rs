use std::collections::VecDeque;
use std::io;

use actix::prelude::*;
use actix_http::http::Uri;
use actix_rt::net::TcpStream;
use actix_service::boxed::{service, BoxService};
use actix_tls::connect::{self, Connect, ConnectError, Connection};
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use log::{error, info, warn};
use redis_async::error::Error as RespError;
use redis_async::resp::{RespCodec, RespValue};
use redis_async::resp_array;
use tokio::io::{split, WriteHalf};
use tokio::sync::oneshot;
use tokio_util::codec::FramedRead;

use crate::Error;

/// Command for send data to Redis
#[derive(Debug)]
pub struct Command(pub RespValue);

impl Message for Command {
    type Result = Result<RespValue, Error>;
}

#[cfg(not(feature = "native-tls"))]
pub type RedisStream = TcpStream;
#[cfg(feature = "native-tls")]
pub type RedisStream = tokio_native_tls::TlsStream<TcpStream>;

/// Redis communication actor
pub struct RedisActor {
    uri: Uri,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    connector: BoxService<Connect<Uri>, Connection<Uri, TcpStream>, ConnectError>,
    tls_connector:
        BoxService<Connection<Uri, TcpStream>, Connection<Uri, RedisStream>, io::Error>,
    backoff: ExponentialBackoff,
    cell: Option<actix::io::FramedWrite<RespValue, WriteHalf<RedisStream>, RespCodec>>,
    queue: VecDeque<oneshot::Sender<Result<RespValue, Error>>>,
}

impl RedisActor {
    /// Start new `Supervisor` with `RedisActor`.
    pub fn start(addr: impl Into<String>) -> Addr<RedisActor> {
        let addr = addr.into();

        // TODO: return error for preparing uri and connectors
        let uri = <Uri as std::str::FromStr>::from_str(addr.as_str()).unwrap();

        // this is a lazy way to get username and password from input.
        // A homebrew extractor can be introduced if reduce dep tree is priority.
        let url = url::Url::parse(addr.as_str()).unwrap();
        let username = if url.username() == "" {
            None
        } else {
            Some(url.username().to_owned())
        };

        let password = url.password().map(ToOwned::to_owned);

        let port = url.port().unwrap_or(6379);
        let scheme = url.scheme();

        let tls_connector = match scheme {
            #[cfg(not(feature = "native-tls"))]
            "redis" => service(actix_service::fn_service(|req| async { Ok(req) })),
            #[cfg(feature = "native-tls")]
            "rediss" => {
                let connector = tokio_native_tls::TlsConnector::from(
                    tokio_native_tls::native_tls::TlsConnector::builder()
                        // TODO: make these flags configurable through session builder.
                        .danger_accept_invalid_certs(true)
                        .danger_accept_invalid_hostnames(true)
                        .use_sni(false)
                        .build()
                        .unwrap(),
                );

                service(actix_service::fn_service(
                    move |req: Connection<Uri, TcpStream>| {
                        let (stream, addr) = req.into_parts();
                        let connector = connector.clone();
                        async move {
                            let res = connector
                                .connect(addr.host().unwrap(), stream)
                                .await
                                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                            Ok(Connection::from_parts(res, addr))
                        }
                    },
                ))
            }
            "redis+unix" | "unix" => panic!("unix domain socket is not supported"),
            _ => panic!("Feature not support"),
        };

        let backoff = ExponentialBackoff {
            max_elapsed_time: None,
            ..Default::default()
        };

        let connector = service(connect::default_connector());

        Supervisor::start(move |_| RedisActor {
            uri,
            port,
            username,
            password,
            connector,
            tls_connector,
            cell: None,
            backoff,
            queue: VecDeque::new(),
        })
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let req = Connect::new(self.uri.to_owned()).set_port(self.port);
        self.connector
            .call(req)
            .into_actor(self)
            .map(|res, act, _| res.map(|conn| act.tls_connector.call(conn)))
            .then(|res, act, _| {
                async { res?.await.map_err(ConnectError::Io) }.into_actor(act)
            })
            .map(|res, act, ctx| match res {
                Ok(conn) => {
                    let stream = conn.into_parts().0;

                    info!("Connected to redis server: {}", act.uri);

                    let (r, w) = split(stream);

                    // configure write side of the connection
                    let framed = actix::io::FramedWrite::new(w, RespCodec, ctx);
                    act.cell = Some(framed);

                    // read side of the connection
                    ctx.add_stream(FramedRead::new(r, RespCodec));

                    act.backoff.reset();
                }
                Err(err) => {
                    error!("Can not connect to redis server: {}", err);
                    // re-connect with backoff time.
                    // we stop current context, supervisor will restart it.
                    if let Some(timeout) = act.backoff.next_backoff() {
                        ctx.run_later(timeout, |_, ctx| ctx.stop());
                    }
                }
            })
            .wait(ctx);

        // There is a limitation for this pattern. That is this could not be the first message
        // get through actor and there could be other message sent too soon to get ahead and
        // get false alarm of not authenticated.
        match (self.username.as_ref(), self.password.as_ref()) {
            (Some(username), Some(password)) => ctx
                .address()
                .send(Command(resp_array!["AUTH", username, password]))
                .into_actor(self)
                .map(|res, _, _| {
                    // TODO: handle authentication error.
                    if let RespValue::Error(e) = res.unwrap().unwrap() {
                        panic!(e);
                    }
                })
                .spawn(ctx),
            (None, Some(password)) => ctx
                .address()
                .send(Command(resp_array!["AUTH", password]))
                .into_actor(self)
                .map(|res, _, _| {
                    // TODO: handle authentication error.
                    if let RespValue::Error(e) = res.unwrap().unwrap() {
                        panic!(e);
                    }
                })
                .spawn(ctx),
            _ => {}
        }
    }
}

impl Supervised for RedisActor {
    fn restarting(&mut self, _: &mut Self::Context) {
        self.cell.take();
        for tx in self.queue.drain(..) {
            let _ = tx.send(Err(Error::Disconnected));
        }
    }
}

impl actix::io::WriteHandler<io::Error> for RedisActor {
    fn error(&mut self, err: io::Error, _: &mut Self::Context) -> Running {
        warn!("Redis connection dropped: {} error: {}", self.uri, err);
        Running::Stop
    }
}

impl StreamHandler<Result<RespValue, RespError>> for RedisActor {
    fn handle(&mut self, msg: Result<RespValue, RespError>, ctx: &mut Self::Context) {
        match msg {
            Err(e) => {
                if let Some(tx) = self.queue.pop_front() {
                    let _ = tx.send(Err(e.into()));
                }
                ctx.stop();
            }
            Ok(val) => {
                if let Some(tx) = self.queue.pop_front() {
                    let _ = tx.send(Ok(val));
                }
            }
        }
    }
}

impl Handler<Command> for RedisActor {
    type Result = ResponseFuture<Result<RespValue, Error>>;

    fn handle(&mut self, msg: Command, _: &mut Self::Context) -> Self::Result {
        let (tx, rx) = oneshot::channel();
        if let Some(ref mut cell) = self.cell {
            self.queue.push_back(tx);
            cell.write(msg.0);
        } else {
            let _ = tx.send(Err(Error::NotConnected));
        }

        Box::pin(async move { rx.await.map_err(|_| Error::Disconnected)? })
    }
}
