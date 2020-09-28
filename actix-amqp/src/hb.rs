use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use actix_rt::time::{delay_until, Delay, Instant};
use actix_utils::time::LowResTimeService;

use crate::errors::AmqpTransportError;

pub(crate) enum HeartbeatAction {
    None,
    Heartbeat,
    Close,
}

pub(crate) struct Heartbeat {
    expire_local: Instant,
    expire_remote: Instant,
    local: Duration,
    remote: Option<Duration>,
    time: LowResTimeService,
    delay: Delay,
}

impl Heartbeat {
    pub(crate) fn new(local: Duration, remote: Option<Duration>, time: LowResTimeService) -> Self {
        let now = Instant::from_std(time.now());
        let delay = if let Some(remote) = remote {
            delay_until(now + std::cmp::min(local, remote))
        } else {
            delay_until(now + local)
        };

        Heartbeat {
            expire_local: now,
            expire_remote: now,
            local,
            remote,
            time,
            delay,
        }
    }

    pub(crate) fn update_local(&mut self, update: bool) {
        if update {
            self.expire_local = Instant::from_std(self.time.now());
        }
    }

    pub(crate) fn update_remote(&mut self, update: bool) {
        if update && self.remote.is_some() {
            self.expire_remote = Instant::from_std(self.time.now());
        }
    }

    fn next_expire(&self) -> Instant {
        if let Some(remote) = self.remote {
            let t1 = self.expire_local + self.local;
            let t2 = self.expire_remote + remote;
            if t1 < t2 {
                t1
            } else {
                t2
            }
        } else {
            self.expire_local + self.local
        }
    }

    pub(crate) fn poll(&mut self, cx: &mut Context) -> Result<HeartbeatAction, AmqpTransportError> {
        match Pin::new(&mut self.delay).poll(cx) {
            Poll::Ready(_) => {
                let mut act = HeartbeatAction::None;
                let dl = self.delay.deadline();
                if dl >= self.expire_local + self.local {
                    // close connection
                    return Ok(HeartbeatAction::Close);
                }
                if let Some(remote) = self.remote {
                    if dl >= self.expire_remote + remote {
                        // send heartbeat
                        act = HeartbeatAction::Heartbeat;
                    }
                }
                self.delay.reset(self.next_expire());
                let _ = Pin::new(&mut self.delay).poll(cx);
                Ok(act)
            }
            Poll::Pending => Ok(HeartbeatAction::None),
        }
    }
}
