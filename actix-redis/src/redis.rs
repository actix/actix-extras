use std::collections::VecDeque;
use std::io;

use actix::prelude::*;
use actix_rt::net::TcpStream;
use actix_service::boxed::{self, BoxService};
use actix_tls::connect::{ConnectError, ConnectInfo, Connection, ConnectorService};
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use log::{error, info, warn};
use redis_async::error::Error as RespError;
use redis_async::resp::{RespCodec, RespValue};
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

/// Redis communication actor
pub struct RedisActor {
    addr: String,
    connector: BoxService<ConnectInfo<String>, Connection<String, TcpStream>, ConnectError>,
    backoff: ExponentialBackoff,
    cell: Option<actix::io::FramedWrite<RespValue, WriteHalf<TcpStream>, RespCodec>>,
    queue: VecDeque<oneshot::Sender<Result<RespValue, Error>>>,
}

impl RedisActor {
    /// Start new `Supervisor` with `RedisActor`.
    pub fn start<S: Into<String>>(addr: S) -> Addr<RedisActor> {
        let addr = addr.into();

        let backoff = ExponentialBackoff {
            max_elapsed_time: None,
            ..Default::default()
        };

        Supervisor::start(|_| RedisActor {
            addr,
            connector: boxed::service(ConnectorService::default()),
            cell: None,
            backoff,
            queue: VecDeque::new(),
        })
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let req = ConnectInfo::new(self.addr.to_owned());
        self.connector
            .call(req)
            .into_actor(self)
            .map(|res, act, ctx| match res {
                Ok(conn) => {
                    let stream = conn.into_parts().0;
                    info!("Connected to redis server: {}", act.addr);

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
        warn!("Redis connection dropped: {} error: {}", self.addr, err);
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
