use std::collections::VecDeque;
use std::io;

use actix::actors::resolver::{Connect, Resolver};
use actix::prelude::*;
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use futures::unsync::oneshot;
use futures::Future;
use redis_async::error::Error as RespError;
use redis_async::resp::{RespCodec, RespValue};
use tokio_codec::FramedRead;
use tokio_io::io::WriteHalf;
use tokio_io::AsyncRead;
use tokio_tcp::TcpStream;

use crate::Error;

/// Command for send data to Redis
#[derive(Debug)]
pub struct Command(pub RespValue);

impl Message for Command {
    type Result = Result<RespValue, Error>;
}

/// Redis comminucation actor
pub struct RedisActor {
    addr: String,
    backoff: ExponentialBackoff,
    cell: Option<actix::io::FramedWrite<WriteHalf<TcpStream>, RespCodec>>,
    queue: VecDeque<oneshot::Sender<Result<RespValue, Error>>>,
}

impl RedisActor {
    /// Start new `Supervisor` with `RedisActor`.
    pub fn start<S: Into<String>>(addr: S) -> Addr<RedisActor> {
        let addr = addr.into();

        let mut backoff = ExponentialBackoff::default();
        backoff.max_elapsed_time = None;

        Supervisor::start(|_| RedisActor {
            addr,
            cell: None,
            backoff: backoff,
            queue: VecDeque::new(),
        })
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        Resolver::from_registry()
            .send(Connect::host(self.addr.as_str()))
            .into_actor(self)
            .map(|res, act, ctx| match res {
                Ok(stream) => {
                    info!("Connected to redis server: {}", act.addr);

                    let (r, w) = stream.split();

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
            .map_err(|err, act, ctx| {
                error!("Can not connect to redis server: {}", err);
                // re-connect with backoff time.
                // we stop current context, supervisor will restart it.
                if let Some(timeout) = act.backoff.next_backoff() {
                    ctx.run_later(timeout, |_, ctx| ctx.stop());
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

impl StreamHandler<RespValue, RespError> for RedisActor {
    fn error(&mut self, err: RespError, _: &mut Self::Context) -> Running {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(Err(err.into()));
        }
        Running::Stop
    }

    fn handle(&mut self, msg: RespValue, _: &mut Self::Context) {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(Ok(msg));
        }
    }
}

impl Handler<Command> for RedisActor {
    type Result = ResponseFuture<RespValue, Error>;

    fn handle(&mut self, msg: Command, _: &mut Self::Context) -> Self::Result {
        let (tx, rx) = oneshot::channel();
        if let Some(ref mut cell) = self.cell {
            self.queue.push_back(tx);
            cell.write(msg.0);
        } else {
            let _ = tx.send(Err(Error::NotConnected));
        }

        Box::new(rx.map_err(|_| Error::Disconnected).and_then(|res| res))
    }
}
