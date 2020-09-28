use std::fmt;

use amqp_codec::protocol::{Accepted, DeliveryState, Error, Rejected, Transfer, TransferBody};
use amqp_codec::Decode;
use bytes::Bytes;

use crate::rcvlink::ReceiverLink;
use crate::session::Session;

use super::errors::AmqpError;
use super::State;

pub struct Message<S> {
    state: State<S>,
    frame: Transfer,
    link: ReceiverLink,
}

#[derive(Debug)]
pub enum Outcome {
    Accept,
    Reject,
    Error(Error),
}

impl<T> From<T> for Outcome
where
    T: Into<Error>,
{
    fn from(err: T) -> Self {
        Outcome::Error(err.into())
    }
}

impl Outcome {
    pub(crate) fn into_delivery_state(self) -> DeliveryState {
        match self {
            Outcome::Accept => DeliveryState::Accepted(Accepted {}),
            Outcome::Reject => DeliveryState::Rejected(Rejected { error: None }),
            Outcome::Error(e) => DeliveryState::Rejected(Rejected { error: Some(e) }),
        }
    }
}

impl<S> Message<S> {
    pub(crate) fn new(state: State<S>, frame: Transfer, link: ReceiverLink) -> Self {
        Message { state, frame, link }
    }

    pub fn state(&self) -> &S {
        self.state.get_ref()
    }

    pub fn state_mut(&mut self) -> &mut S {
        self.state.get_mut()
    }

    pub fn session(&self) -> &Session {
        self.link.session()
    }

    pub fn session_mut(&mut self) -> &mut Session {
        self.link.session_mut()
    }

    pub fn frame(&self) -> &Transfer {
        &self.frame
    }

    pub fn body(&self) -> Option<&Bytes> {
        match self.frame.body {
            Some(TransferBody::Data(ref b)) => Some(b),
            _ => None,
        }
    }

    pub fn load_message<T: Decode>(&self) -> Result<T, AmqpError> {
        if let Some(TransferBody::Data(ref b)) = self.frame.body {
            if let Ok((_, msg)) = T::decode(b) {
                Ok(msg)
            } else {
                Err(AmqpError::decode_error().description("Can not decode message"))
            }
        } else {
            Err(AmqpError::invalid_field().description("Unknown body"))
        }
    }
}

impl<S> fmt::Debug for Message<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Message<S>")
            .field("frame", &self.frame)
            .finish()
    }
}
