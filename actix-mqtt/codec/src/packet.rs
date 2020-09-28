use bytes::Bytes;
use bytestring::ByteString;
use std::num::NonZeroU16;

use crate::proto::{Protocol, QoS};

#[repr(u8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
/// Connect Return Code
pub enum ConnectCode {
    /// Connection accepted
    ConnectionAccepted = 0,
    /// Connection Refused, unacceptable protocol version
    UnacceptableProtocolVersion = 1,
    /// Connection Refused, identifier rejected
    IdentifierRejected = 2,
    /// Connection Refused, Server unavailable
    ServiceUnavailable = 3,
    /// Connection Refused, bad user name or password
    BadUserNameOrPassword = 4,
    /// Connection Refused, not authorized
    NotAuthorized = 5,
    /// Reserved
    Reserved = 6,
}

const_enum!(ConnectCode: u8);

impl ConnectCode {
    pub fn reason(self) -> &'static str {
        match self {
            ConnectCode::ConnectionAccepted => "Connection Accepted",
            ConnectCode::UnacceptableProtocolVersion => {
                "Connection Refused, unacceptable protocol version"
            }
            ConnectCode::IdentifierRejected => "Connection Refused, identifier rejected",
            ConnectCode::ServiceUnavailable => "Connection Refused, Server unavailable",
            ConnectCode::BadUserNameOrPassword => {
                "Connection Refused, bad user name or password"
            }
            ConnectCode::NotAuthorized => "Connection Refused, not authorized",
            _ => "Connection Refused",
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
/// Connection Will
pub struct LastWill {
    /// the QoS level to be used when publishing the Will Message.
    pub qos: QoS,
    /// the Will Message is to be Retained when it is published.
    pub retain: bool,
    /// the Will Topic
    pub topic: ByteString,
    /// defines the Application Message that is to be published to the Will Topic
    pub message: Bytes,
}

#[derive(Debug, PartialEq, Clone)]
/// Connect packet content
pub struct Connect {
    /// mqtt protocol version
    pub protocol: Protocol,
    /// the handling of the Session state.
    pub clean_session: bool,
    /// a time interval measured in seconds.
    pub keep_alive: u16,
    /// Will Message be stored on the Server and associated with the Network Connection.
    pub last_will: Option<LastWill>,
    /// identifies the Client to the Server.
    pub client_id: ByteString,
    /// username can be used by the Server for authentication and authorization.
    pub username: Option<ByteString>,
    /// password can be used by the Server for authentication and authorization.
    pub password: Option<Bytes>,
}

#[derive(Debug, PartialEq, Clone)]
/// Publish message
pub struct Publish {
    /// this might be re-delivery of an earlier attempt to send the Packet.
    pub dup: bool,
    pub retain: bool,
    /// the level of assurance for delivery of an Application Message.
    pub qos: QoS,
    /// the information channel to which payload data is published.
    pub topic: ByteString,
    /// only present in PUBLISH Packets where the QoS level is 1 or 2.
    pub packet_id: Option<NonZeroU16>,
    /// the Application Message that is being published.
    pub payload: Bytes,
}

#[derive(Debug, PartialEq, Copy, Clone)]
/// Subscribe Return Code
pub enum SubscribeReturnCode {
    Success(QoS),
    Failure,
}

#[derive(Debug, PartialEq, Clone)]
/// MQTT Control Packets
pub enum Packet {
    /// Client request to connect to Server
    Connect(Connect),

    /// Connect acknowledgment
    ConnectAck {
        /// enables a Client to establish whether the Client and Server have a consistent view
        /// about whether there is already stored Session state.
        session_present: bool,
        return_code: ConnectCode,
    },

    /// Publish message
    Publish(Publish),

    /// Publish acknowledgment
    PublishAck {
        /// Packet Identifier
        packet_id: NonZeroU16,
    },
    /// Publish received (assured delivery part 1)
    PublishReceived {
        /// Packet Identifier
        packet_id: NonZeroU16,
    },
    /// Publish release (assured delivery part 2)
    PublishRelease {
        /// Packet Identifier
        packet_id: NonZeroU16,
    },
    /// Publish complete (assured delivery part 3)
    PublishComplete {
        /// Packet Identifier
        packet_id: NonZeroU16,
    },

    /// Client subscribe request
    Subscribe {
        /// Packet Identifier
        packet_id: NonZeroU16,
        /// the list of Topic Filters and QoS to which the Client wants to subscribe.
        topic_filters: Vec<(ByteString, QoS)>,
    },
    /// Subscribe acknowledgment
    SubscribeAck {
        packet_id: NonZeroU16,
        /// corresponds to a Topic Filter in the SUBSCRIBE Packet being acknowledged.
        status: Vec<SubscribeReturnCode>,
    },

    /// Unsubscribe request
    Unsubscribe {
        /// Packet Identifier
        packet_id: NonZeroU16,
        /// the list of Topic Filters that the Client wishes to unsubscribe from.
        topic_filters: Vec<ByteString>,
    },
    /// Unsubscribe acknowledgment
    UnsubscribeAck {
        /// Packet Identifier
        packet_id: NonZeroU16,
    },

    /// PING request
    PingRequest,
    /// PING response
    PingResponse,

    /// Client is disconnecting
    Disconnect,
}

impl Packet {
    #[inline]
    /// MQTT Control Packet type
    pub fn packet_type(&self) -> u8 {
        match *self {
            Packet::Connect { .. } => CONNECT,
            Packet::ConnectAck { .. } => CONNACK,
            Packet::Publish { .. } => PUBLISH,
            Packet::PublishAck { .. } => PUBACK,
            Packet::PublishReceived { .. } => PUBREC,
            Packet::PublishRelease { .. } => PUBREL,
            Packet::PublishComplete { .. } => PUBCOMP,
            Packet::Subscribe { .. } => SUBSCRIBE,
            Packet::SubscribeAck { .. } => SUBACK,
            Packet::Unsubscribe { .. } => UNSUBSCRIBE,
            Packet::UnsubscribeAck { .. } => UNSUBACK,
            Packet::PingRequest => PINGREQ,
            Packet::PingResponse => PINGRESP,
            Packet::Disconnect => DISCONNECT,
        }
    }

    /// Flags specific to each MQTT Control Packet type
    pub fn packet_flags(&self) -> u8 {
        match *self {
            Packet::Publish(Publish {
                dup, qos, retain, ..
            }) => {
                let mut b = qos as u8;

                b <<= 1;

                if dup {
                    b |= 0b1000;
                }

                if retain {
                    b |= 0b0001;
                }

                b
            }
            Packet::PublishRelease { .. }
            | Packet::Subscribe { .. }
            | Packet::Unsubscribe { .. } => 0b0010,
            _ => 0,
        }
    }
}

impl From<Connect> for Packet {
    fn from(val: Connect) -> Packet {
        Packet::Connect(val)
    }
}

impl From<Publish> for Packet {
    fn from(val: Publish) -> Packet {
        Packet::Publish(val)
    }
}

pub const CONNECT: u8 = 1;
pub const CONNACK: u8 = 2;
pub const PUBLISH: u8 = 3;
pub const PUBACK: u8 = 4;
pub const PUBREC: u8 = 5;
pub const PUBREL: u8 = 6;
pub const PUBCOMP: u8 = 7;
pub const SUBSCRIBE: u8 = 8;
pub const SUBACK: u8 = 9;
pub const UNSUBSCRIBE: u8 = 10;
pub const UNSUBACK: u8 = 11;
pub const PINGREQ: u8 = 12;
pub const PINGRESP: u8 = 13;
pub const DISCONNECT: u8 = 14;
