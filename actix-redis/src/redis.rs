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
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::Error;

use crate::command::RedisCommand;
use actix_service::Service;
use futures::{FutureExt, SinkExt};

/// Command for send data to Redis
#[derive(Debug)]
pub struct Command(pub RespValue);

impl Message for Command {
    type Result = Result<RespValue, Error>;
}

#[derive(Debug)]
struct WriterError(io::Error);

impl Message for WriterError {
    type Result = ();
}

/// Redis communication actor
pub struct RedisActor {
    addr: String,
    connector: BoxService<ConnectInfo<String>, Connection<String, TcpStream>, ConnectError>,
    backoff: ExponentialBackoff,
    cell: Option<mpsc::UnboundedSender<RespValue>>,
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
                    let (tx, rx) = mpsc::unbounded_channel();

                    act.cell = Some(tx);

                    // read side of the connection
                    ctx.add_stream(FramedRead::new(r, RespCodec));
                    spawn_writer(ctx.address(), w, rx);

                    act.backoff.reset();
                }
                Err(err) => {
                    error!("Can not connect to redis server({}): {}", act.addr, err);
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

impl Handler<WriterError> for RedisActor {
    type Result = ();

    fn handle(&mut self, msg: WriterError, ctx: &mut Self::Context) {
        self.cell.take();
        warn!("Redis connection dropped: {} error: {}", self.addr, msg.0);
        ctx.stop();
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
            if cell.send(msg.0).is_ok() {
                self.queue.push_back(tx);
            } else {
                self.cell.take();
                let _ = tx.send(Err(Error::Disconnected));
            }
        } else {
            let _ = tx.send(Err(Error::NotConnected));
        }

        Box::pin(async move { rx.await.map_err(|_| Error::Disconnected)? })
    }
}

impl<T> Handler<T> for RedisActor
where
    T: RedisCommand + Message<Result = Result<<T as RedisCommand>::Output, Error>>,
    T::Output: Send + 'static,
{
    type Result = ResponseFuture<Result<T::Output, Error>>;

    fn handle(&mut self, msg: T, _: &mut Self::Context) -> Self::Result {
        let (tx, rx) = oneshot::channel();
        if let Some(ref mut cell) = self.cell {
            let msg = msg.serialize();
            if cell.send(msg).is_ok() {
                self.queue.push_back(tx);
            } else {
                self.cell.take();
                let _ = tx.send(Err(Error::Disconnected));
            }
        } else {
            let _ = tx.send(Err(Error::NotConnected));
        }

        Box::pin(rx.map(|res| match res {
            Ok(Ok(resp)) => match T::deserialize(resp) {
                Ok(output) => Ok(output),
                Err(e) => Err(Error::Redis(RespError::Resp(e.message, e.resp))),
            },
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::Disconnected),
        }))
    }
}

fn spawn_writer(
    addr: Addr<RedisActor>,
    writer: WriteHalf<TcpStream>,
    mut rx: mpsc::UnboundedReceiver<RespValue>,
) {
    actix_rt::spawn(async move {
        let mut framed = FramedWrite::new(writer, RespCodec);

        while let Some(msg) = rx.recv().await {
            if let Err(err) = framed.send(msg).await {
                addr.do_send(WriterError(err));
                return;
            }
        }
    });
}
