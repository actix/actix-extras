mod body;
mod inmessage;
mod outmessage;

pub use self::body::MessageBody;
pub use self::inmessage::InMessage;
pub use self::outmessage::OutMessage;

pub(self) const SECTION_PREFIX_LENGTH: usize = 3;
