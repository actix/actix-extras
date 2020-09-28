use actix_service::boxed::{BoxService, BoxServiceFactory};
use amqp_codec::protocol;

use crate::cell::Cell;
use crate::rcvlink::ReceiverLink;
use crate::session::Session;
use crate::sndlink::SenderLink;

use super::errors::LinkError;
use super::State;

pub(crate) type ControlFrameService<St> = BoxService<ControlFrame<St>, (), LinkError>;
pub(crate) type ControlFrameNewService<St> =
    BoxServiceFactory<(), ControlFrame<St>, (), LinkError, ()>;

pub struct ControlFrame<St>(pub(super) Cell<FrameInner<St>>);

pub(super) struct FrameInner<St> {
    pub(super) kind: ControlFrameKind,
    pub(super) state: State<St>,
    pub(super) session: Session,
}

#[derive(Debug)]
pub enum ControlFrameKind {
    Attach(protocol::Attach),
    Flow(protocol::Flow, SenderLink),
    DetachSender(protocol::Detach, SenderLink),
    DetachReceiver(protocol::Detach, ReceiverLink),
}

impl<St> ControlFrame<St> {
    pub(crate) fn new(state: State<St>, session: Session, kind: ControlFrameKind) -> Self {
        ControlFrame(Cell::new(FrameInner {
            state,
            session,
            kind,
        }))
    }

    pub(crate) fn clone(&self) -> Self {
        ControlFrame(self.0.clone())
    }

    #[inline]
    pub fn state(&self) -> &St {
        self.0.state.get_ref()
    }

    #[inline]
    pub fn state_mut(&mut self) -> &mut St {
        self.0.get_mut().state.get_mut()
    }

    #[inline]
    pub fn session(&self) -> &Session {
        &self.0.session
    }

    #[inline]
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.0.get_mut().session
    }

    #[inline]
    pub fn frame(&self) -> &ControlFrameKind {
        &self.0.kind
    }
}
