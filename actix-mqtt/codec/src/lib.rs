#[macro_use]
extern crate bitflags;

extern crate bytestring;

mod error;
#[macro_use]
mod topic;
#[macro_use]
mod proto;
mod codec;
mod packet;

pub use self::codec::Codec;
pub use self::error::{ParseError, TopicError};
pub use self::packet::{Connect, ConnectCode, LastWill, Packet, Publish, SubscribeReturnCode};
pub use self::proto::{Protocol, QoS};
pub use self::topic::{Level, Topic};

// http://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.xhtml
pub const TCP_PORT: u16 = 1883;
pub const SSL_PORT: u16 = 8883;
