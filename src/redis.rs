use std::io;
use std::collections::VecDeque;

use actix::prelude::*;
use backoff::ExponentialBackoff;
use backoff::backoff::Backoff;
use futures::Future;
use futures::unsync::oneshot;
use tokio_io::AsyncRead;
use tokio_core::net::TcpStream;
use redis_async::{resp, error};

use connect::TcpConnector;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display="Redis error {}", _0)]
    Redis(error::Error),
    /// Receiving message during reconnecting
    #[fail(display="Redis: Not connected")]
    NotConnected,
    /// Cancel all waters when connection get dropped
    #[fail(display="Redis: Disconnected")]
    Disconnected,
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl From<error::Error> for Error {
    fn from(err: error::Error) -> Error {
        Error::Redis(err)
    }
}

pub struct Command(pub resp::RespValue);

impl ResponseType for Command {
    type Item = resp::RespValue;
    type Error = Error;
}

/// Redis comminucation actor
pub struct RedisActor {
    addr: String,
    backoff: ExponentialBackoff,
    cell: Option<FramedCell<RedisActor>>,
    queue: VecDeque<oneshot::Sender<Result<resp::RespValue, Error>>>,
}

impl RedisActor {
    pub fn start<S: Into<String>>(addr: S) -> Address<RedisActor> {
        let addr = addr.into();

        Supervisor::start(|_| {
            RedisActor { addr: addr,
                         cell: None,
                         backoff: ExponentialBackoff::default(),
                         queue: VecDeque::new() }
        })
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        TcpConnector::new(self.addr.as_str())
            .into_actor(self)
            .map(|stream, act, ctx| {
                info!("Connected to redis server: {}", act.addr);
                act.backoff.reset();
                act.cell = Some(act.add_framed(stream.framed(resp::RespCodec), ctx));
            })
            .map_err(|err, act, ctx| {
                error!("Can not connect to redis server: {}", err);
                debug!("{:?}", err);
                if let Some(timeout) = act.backoff.next_backoff() {
                    // delay re-connect, drop all messages during this period
                    ctx.run_later(timeout, |_, ctx| {
                        ctx.stop()
                    });
                } else {
                    ctx.stop();
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

impl FramedActor for RedisActor {
    type Io = TcpStream;
    type Codec = resp::RespCodec;

    fn closed(&mut self, error: Option<io::Error>, _: &mut Self::Context) {
        if let Some(err) = error {
            warn!("Redis connection dropped: {} error: {}", self.addr, err);
        } else {
            warn!("Redis connection dropped: {}", self.addr);
        }
    }

    fn handle(&mut self, msg: Result<resp::RespValue, error::Error>, _ctx: &mut Self::Context) {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(msg.map_err(|e| e.into()));
        }
    }
}

impl Handler<Command> for RedisActor {
    type Result = ResponseFuture<Self, Command>;

    fn handle(&mut self, msg: Command, _: &mut Self::Context) -> Self::Result {
        let (tx, rx) = oneshot::channel();
        if let Some(ref mut cell) = self.cell {
            self.queue.push_back(tx);
            cell.send(msg.0);
        } else {
            let _ = tx.send(Err(Error::NotConnected));
        }

        Box::new(
            rx.map_err(|_| Error::Disconnected)
                .and_then(|res| res)
                .actfuture())
    }
}
