use super::protocol;

/// Length in bytes of the fixed frame header
pub const HEADER_LEN: usize = 8;

/// AMQP Frame type marker (0)
pub const FRAME_TYPE_AMQP: u8 = 0x00;
pub const FRAME_TYPE_SASL: u8 = 0x01;

/// Represents an AMQP Frame
#[derive(Clone, Debug, PartialEq)]
pub struct AmqpFrame {
    channel_id: u16,
    performative: protocol::Frame,
}

impl AmqpFrame {
    pub fn new(channel_id: u16, performative: protocol::Frame) -> AmqpFrame {
        AmqpFrame {
            channel_id,
            performative,
        }
    }

    #[inline]
    pub fn channel_id(&self) -> u16 {
        self.channel_id
    }

    #[inline]
    pub fn performative(&self) -> &protocol::Frame {
        &self.performative
    }

    #[inline]
    pub fn into_parts(self) -> (u16, protocol::Frame) {
        (self.channel_id, self.performative)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaslFrame {
    pub body: protocol::SaslFrameBody,
}

impl SaslFrame {
    pub fn new(body: protocol::SaslFrameBody) -> SaslFrame {
        SaslFrame { body }
    }
}

impl From<protocol::SaslMechanisms> for SaslFrame {
    fn from(item: protocol::SaslMechanisms) -> SaslFrame {
        SaslFrame::new(protocol::SaslFrameBody::SaslMechanisms(item))
    }
}

impl From<protocol::SaslInit> for SaslFrame {
    fn from(item: protocol::SaslInit) -> SaslFrame {
        SaslFrame::new(protocol::SaslFrameBody::SaslInit(item))
    }
}

impl From<protocol::SaslChallenge> for SaslFrame {
    fn from(item: protocol::SaslChallenge) -> SaslFrame {
        SaslFrame::new(protocol::SaslFrameBody::SaslChallenge(item))
    }
}

impl From<protocol::SaslResponse> for SaslFrame {
    fn from(item: protocol::SaslResponse) -> SaslFrame {
        SaslFrame::new(protocol::SaslFrameBody::SaslResponse(item))
    }
}

impl From<protocol::SaslOutcome> for SaslFrame {
    fn from(item: protocol::SaslOutcome) -> SaslFrame {
        SaslFrame::new(protocol::SaslFrameBody::SaslOutcome(item))
    }
}
