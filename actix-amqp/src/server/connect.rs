use actix_codec::{AsyncRead, AsyncWrite, Framed};
use amqp_codec::protocol::{Frame, Open};
use amqp_codec::{AmqpCodec, AmqpFrame, ProtocolIdCodec};
use futures::{Future, StreamExt};

use super::errors::ServerError;
use crate::connection::ConnectionController;

/// Open new connection
pub struct Connect<Io> {
    conn: Framed<Io, ProtocolIdCodec>,
    controller: ConnectionController,
}

impl<Io> Connect<Io> {
    pub(crate) fn new(conn: Framed<Io, ProtocolIdCodec>, controller: ConnectionController) -> Self {
        Self { conn, controller }
    }

    /// Returns reference to io object
    pub fn get_ref(&self) -> &Io {
        self.conn.get_ref()
    }

    /// Returns mutable reference to io object
    pub fn get_mut(&mut self) -> &mut Io {
        self.conn.get_mut()
    }
}

impl<Io: AsyncRead + AsyncWrite> Connect<Io> {
    /// Wait for connection open frame
    pub async fn open(self) -> Result<ConnectOpened<Io>, ServerError<()>> {
        let mut framed = self.conn.into_framed(AmqpCodec::<AmqpFrame>::new());
        let mut controller = self.controller;

        let frame = framed
            .next()
            .await
            .ok_or(ServerError::Disconnected)?
            .map_err(ServerError::from)?;

        let frame = frame.into_parts().1;
        match frame {
            Frame::Open(frame) => {
                trace!("Got open frame: {:?}", frame);
                controller.set_remote((&frame).into());
                Ok(ConnectOpened {
                    frame,
                    framed,
                    controller,
                })
            }
            frame => Err(ServerError::Unexpected(frame)),
        }
    }
}

/// Connection is opened
pub struct ConnectOpened<Io> {
    frame: Open,
    framed: Framed<Io, AmqpCodec<AmqpFrame>>,
    controller: ConnectionController,
}

impl<Io> ConnectOpened<Io> {
    pub(crate) fn new(
        frame: Open,
        framed: Framed<Io, AmqpCodec<AmqpFrame>>,
        controller: ConnectionController,
    ) -> Self {
        ConnectOpened {
            frame,
            framed,
            controller,
        }
    }

    /// Get reference to remote `Open` frame
    pub fn frame(&self) -> &Open {
        &self.frame
    }

    /// Returns reference to io object
    pub fn get_ref(&self) -> &Io {
        self.framed.get_ref()
    }

    /// Returns mutable reference to io object
    pub fn get_mut(&mut self) -> &mut Io {
        self.framed.get_mut()
    }

    /// Connection controller
    pub fn connection(&self) -> &ConnectionController {
        &self.controller
    }

    /// Ack connect message and set state
    pub fn ack<St>(self, state: St) -> ConnectAck<Io, St> {
        ConnectAck {
            state,
            framed: self.framed,
            controller: self.controller,
        }
    }
}

/// Ack connect message
pub struct ConnectAck<Io, St> {
    state: St,
    framed: Framed<Io, AmqpCodec<AmqpFrame>>,
    controller: ConnectionController,
}

impl<Io, St> ConnectAck<Io, St> {
    pub(crate) fn into_inner(self) -> (St, Framed<Io, AmqpCodec<AmqpFrame>>, ConnectionController) {
        (self.state, self.framed, self.controller)
    }
}
