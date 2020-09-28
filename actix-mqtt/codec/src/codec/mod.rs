use actix_codec::{Decoder, Encoder};
use bytes::{buf::Buf, BytesMut};

use crate::error::ParseError;
use crate::proto::QoS;
use crate::{Packet, Publish};

mod decode;
mod encode;

use self::decode::*;
use self::encode::*;

bitflags! {
    pub struct ConnectFlags: u8 {
        const USERNAME      = 0b1000_0000;
        const PASSWORD      = 0b0100_0000;
        const WILL_RETAIN   = 0b0010_0000;
        const WILL_QOS      = 0b0001_1000;
        const WILL          = 0b0000_0100;
        const CLEAN_SESSION = 0b0000_0010;
    }
}

pub const WILL_QOS_SHIFT: u8 = 3;

bitflags! {
    pub struct ConnectAckFlags: u8 {
        const SESSION_PRESENT = 0b0000_0001;
    }
}

#[derive(Debug)]
pub struct Codec {
    state: DecodeState,
    max_size: usize,
}

#[derive(Debug, Clone, Copy)]
enum DecodeState {
    FrameHeader,
    Frame(FixedHeader),
}

impl Codec {
    /// Create `Codec` instance
    pub fn new() -> Self {
        Codec {
            state: DecodeState::FrameHeader,
            max_size: 0,
        }
    }

    /// Set max inbound frame size.
    ///
    /// If max size is set to `0`, size is unlimited.
    /// By default max size is set to `0`
    pub fn max_size(mut self, size: usize) -> Self {
        self.max_size = size;
        self
    }
}

impl Default for Codec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for Codec {
    type Item = Packet;
    type Error = ParseError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, ParseError> {
        loop {
            match self.state {
                DecodeState::FrameHeader => {
                    if src.len() < 2 {
                        return Ok(None);
                    }
                    let fixed = src.as_ref()[0];
                    match decode_variable_length(&src.as_ref()[1..])? {
                        Some((remaining_length, consumed)) => {
                            // check max message size
                            if self.max_size != 0 && self.max_size < remaining_length {
                                return Err(ParseError::MaxSizeExceeded);
                            }
                            src.advance(consumed + 1);
                            self.state = DecodeState::Frame(FixedHeader {
                                packet_type: fixed >> 4,
                                packet_flags: fixed & 0xF,
                                remaining_length,
                            });
                            // todo: validate remaining_length against max frame size config
                            if src.len() < remaining_length {
                                // todo: subtract?
                                src.reserve(remaining_length); // extend receiving buffer to fit the whole frame -- todo: too eager?
                                return Ok(None);
                            }
                        }
                        None => {
                            return Ok(None);
                        }
                    }
                }
                DecodeState::Frame(fixed) => {
                    if src.len() < fixed.remaining_length {
                        return Ok(None);
                    }
                    let packet_buf = src.split_to(fixed.remaining_length);
                    let packet = read_packet(packet_buf.freeze(), fixed)?;
                    self.state = DecodeState::FrameHeader;
                    src.reserve(2);
                    return Ok(Some(packet));
                }
            }
        }
    }
}

impl Encoder for Codec {
    type Item = Packet;
    type Error = ParseError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), ParseError> {
        if let Packet::Publish(Publish { qos, packet_id, .. }) = item {
            if (qos == QoS::AtLeastOnce || qos == QoS::ExactlyOnce) && packet_id.is_none() {
                return Err(ParseError::PacketIdRequired);
            }
        }
        let content_size = get_encoded_size(&item);
        dst.reserve(content_size + 5);
        write_packet(&item, dst, content_size);
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) struct FixedHeader {
    /// MQTT Control Packet type
    pub packet_type: u8,
    /// Flags specific to each MQTT Control Packet type
    pub packet_flags: u8,
    /// the number of bytes remaining within the current packet,
    /// including data in the variable header and the payload.
    pub remaining_length: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_size() {
        let mut codec = Codec::new().max_size(5);

        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"\0\x09");
        assert_eq!(codec.decode(&mut buf), Err(ParseError::MaxSizeExceeded));
    }
}
