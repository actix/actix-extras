mod app;
mod connect;
mod control;
mod dispatcher;
pub mod errors;
mod handshake;
mod link;
mod message;
pub mod sasl;
mod service;

pub use self::app::App;
pub use self::connect::{Connect, ConnectAck, ConnectOpened};
pub use self::control::{ControlFrame, ControlFrameKind};
pub use self::handshake::{handshake, Handshake};
pub use self::link::Link;
pub use self::message::{Message, Outcome};
pub use self::sasl::Sasl;
pub use self::service::Server;

use crate::cell::Cell;

pub struct State<St>(Cell<St>);

impl<St> State<St> {
    pub(crate) fn new(st: St) -> Self {
        State(Cell::new(st))
    }

    pub(crate) fn clone(&self) -> Self {
        State(self.0.clone())
    }

    pub fn get_ref(&self) -> &St {
        self.0.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut St {
        self.0.get_mut()
    }
}

impl<St> std::ops::Deref for State<St> {
    type Target = St;

    fn deref(&self) -> &Self::Target {
        self.get_ref()
    }
}

impl<St> std::ops::DerefMut for State<St> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}
