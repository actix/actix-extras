use std::convert::TryFrom;
use std::num::{NonZeroU16, NonZeroU32};

use bytes::{Buf, Bytes};
use bytestring::ByteString;

use crate::error::ParseError;
use crate::packet::*;
use crate::proto::*;

use super::{ConnectAckFlags, ConnectFlags, FixedHeader, WILL_QOS_SHIFT};

pub(crate) fn read_packet(mut src: Bytes, header: FixedHeader) -> Result<Packet, ParseError> {
    match header.packet_type {
        CONNECT => decode_connect_packet(&mut src),
        CONNACK => decode_connect_ack_packet(&mut src),
        PUBLISH => decode_publish_packet(&mut src, header),
        PUBACK => Ok(Packet::PublishAck {
            packet_id: NonZeroU16::parse(&mut src)?,
        }),
        PUBREC => Ok(Packet::PublishReceived {
            packet_id: NonZeroU16::parse(&mut src)?,
        }),
        PUBREL => Ok(Packet::PublishRelease {
            packet_id: NonZeroU16::parse(&mut src)?,
        }),
        PUBCOMP => Ok(Packet::PublishComplete {
            packet_id: NonZeroU16::parse(&mut src)?,
        }),
        SUBSCRIBE => decode_subscribe_packet(&mut src),
        SUBACK => decode_subscribe_ack_packet(&mut src),
        UNSUBSCRIBE => decode_unsubscribe_packet(&mut src),
        UNSUBACK => Ok(Packet::UnsubscribeAck {
            packet_id: NonZeroU16::parse(&mut src)?,
        }),
        PINGREQ => Ok(Packet::PingRequest),
        PINGRESP => Ok(Packet::PingResponse),
        DISCONNECT => Ok(Packet::Disconnect),
        _ => Err(ParseError::UnsupportedPacketType),
    }
}

macro_rules! check_flag {
    ($flags:expr, $flag:expr) => {
        ($flags & $flag.bits()) == $flag.bits()
    };
}

macro_rules! ensure {
    ($cond:expr, $e:expr) => {
        if !($cond) {
            return Err($e);
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)+) => {
        if !($cond) {
            return Err($fmt, $($arg)+);
        }
    };
}

pub fn decode_variable_length(src: &[u8]) -> Result<Option<(usize, usize)>, ParseError> {
    if let Some((len, consumed, more)) = src
        .iter()
        .enumerate()
        .scan((0, true), |state, (idx, x)| {
            if !state.1 || idx > 3 {
                return None;
            }
            state.0 += ((x & 0x7F) as usize) << (idx * 7);
            state.1 = x & 0x80 != 0;
            Some((state.0, idx + 1, state.1))
        })
        .last()
    {
        ensure!(!more || consumed < 4, ParseError::InvalidLength);
        return Ok(Some((len, consumed)));
    }

    Ok(None)
}

fn decode_connect_packet(src: &mut Bytes) -> Result<Packet, ParseError> {
    ensure!(src.remaining() >= 10, ParseError::InvalidLength);
    let len = src.get_u16();

    ensure!(
        len == 4 && &src.bytes()[0..4] == b"MQTT",
        ParseError::InvalidProtocol
    );
    src.advance(4);

    let level = src.get_u8();
    ensure!(
        level == DEFAULT_MQTT_LEVEL,
        ParseError::UnsupportedProtocolLevel
    );

    let flags = src.get_u8();
    ensure!((flags & 0x01) == 0, ParseError::ConnectReservedFlagSet);

    let keep_alive = src.get_u16();
    let client_id = decode_utf8_str(src)?;

    ensure!(
        !client_id.is_empty() || check_flag!(flags, ConnectFlags::CLEAN_SESSION),
        ParseError::InvalidClientId
    );

    let topic = if check_flag!(flags, ConnectFlags::WILL) {
        Some(decode_utf8_str(src)?)
    } else {
        None
    };
    let message = if check_flag!(flags, ConnectFlags::WILL) {
        Some(decode_length_bytes(src)?)
    } else {
        None
    };
    let username = if check_flag!(flags, ConnectFlags::USERNAME) {
        Some(decode_utf8_str(src)?)
    } else {
        None
    };
    let password = if check_flag!(flags, ConnectFlags::PASSWORD) {
        Some(decode_length_bytes(src)?)
    } else {
        None
    };
    let last_will = if let Some(topic) = topic {
        Some(LastWill {
            qos: QoS::from((flags & ConnectFlags::WILL_QOS.bits()) >> WILL_QOS_SHIFT),
            retain: check_flag!(flags, ConnectFlags::WILL_RETAIN),
            topic,
            message: message.unwrap(),
        })
    } else {
        None
    };

    Ok(Packet::Connect(Connect {
        protocol: Protocol::MQTT(level),
        clean_session: check_flag!(flags, ConnectFlags::CLEAN_SESSION),
        keep_alive,
        client_id,
        last_will,
        username,
        password,
    }))
}

fn decode_connect_ack_packet(src: &mut Bytes) -> Result<Packet, ParseError> {
    ensure!(src.remaining() >= 2, ParseError::InvalidLength);
    let flags = src.get_u8();
    ensure!(
        (flags & 0b1111_1110) == 0,
        ParseError::ConnAckReservedFlagSet
    );

    let return_code = src.get_u8();
    Ok(Packet::ConnectAck {
        session_present: check_flag!(flags, ConnectAckFlags::SESSION_PRESENT),
        return_code: ConnectCode::from(return_code),
    })
}

fn decode_publish_packet(src: &mut Bytes, header: FixedHeader) -> Result<Packet, ParseError> {
    let topic = decode_utf8_str(src)?;
    let qos = QoS::from((header.packet_flags & 0b0110) >> 1);
    let packet_id = if qos == QoS::AtMostOnce {
        None
    } else {
        Some(NonZeroU16::parse(src)?)
    };

    let len = src.remaining();
    let payload = src.split_to(len);

    Ok(Packet::Publish(Publish {
        dup: (header.packet_flags & 0b1000) == 0b1000,
        qos,
        retain: (header.packet_flags & 0b0001) == 0b0001,
        topic,
        packet_id,
        payload,
    }))
}

fn decode_subscribe_packet(src: &mut Bytes) -> Result<Packet, ParseError> {
    let packet_id = NonZeroU16::parse(src)?;
    let mut topic_filters = Vec::new();
    while src.remaining() > 0 {
        let topic = decode_utf8_str(src)?;
        ensure!(src.remaining() >= 1, ParseError::InvalidLength);
        let qos = QoS::from(src.get_u8() & 0x03);
        topic_filters.push((topic, qos));
    }

    Ok(Packet::Subscribe {
        packet_id,
        topic_filters,
    })
}

fn decode_subscribe_ack_packet(src: &mut Bytes) -> Result<Packet, ParseError> {
    let packet_id = NonZeroU16::parse(src)?;
    let status = src
        .bytes()
        .iter()
        .map(|code| {
            if *code == 0x80 {
                SubscribeReturnCode::Failure
            } else {
                SubscribeReturnCode::Success(QoS::from(code & 0x03))
            }
        })
        .collect();
    Ok(Packet::SubscribeAck { packet_id, status })
}

fn decode_unsubscribe_packet(src: &mut Bytes) -> Result<Packet, ParseError> {
    let packet_id = NonZeroU16::parse(src)?;
    let mut topic_filters = Vec::new();
    while src.remaining() > 0 {
        topic_filters.push(decode_utf8_str(src)?);
    }
    Ok(Packet::Unsubscribe {
        packet_id,
        topic_filters,
    })
}

fn decode_length_bytes(src: &mut Bytes) -> Result<Bytes, ParseError> {
    let len: u16 = NonZeroU16::parse(src)?.into();
    ensure!(src.remaining() >= len as usize, ParseError::InvalidLength);
    Ok(src.split_to(len as usize))
}

fn decode_utf8_str(src: &mut Bytes) -> Result<ByteString, ParseError> {
    Ok(ByteString::try_from(decode_length_bytes(src)?)?)
}

pub(crate) trait ByteBuf: Buf {
    fn inner_mut(&mut self) -> &mut Bytes;
}

impl ByteBuf for Bytes {
    fn inner_mut(&mut self) -> &mut Bytes {
        self
    }
}

impl ByteBuf for bytes::buf::ext::Take<&mut Bytes> {
    fn inner_mut(&mut self) -> &mut Bytes {
        self.get_mut()
    }
}

pub(crate) trait Parse: Sized {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError>;
}

impl Parse for bool {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        ensure!(src.has_remaining(), ParseError::InvalidLength); // expected more data within the field
        let v = src.get_u8();
        ensure!(v <= 0x1, ParseError::MalformedPacket); // value is invalid
        Ok(v == 0x1)
    }
}

impl Parse for u16 {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        ensure!(src.remaining() >= 2, ParseError::InvalidLength);
        Ok(src.get_u16())
    }
}

impl Parse for u32 {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        ensure!(src.remaining() >= 4, ParseError::InvalidLength); // expected more data within the field
        let val = src.get_u32();
        Ok(val)
    }
}

impl Parse for NonZeroU32 {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        let val = NonZeroU32::new(u32::parse(src)?).ok_or(ParseError::MalformedPacket)?;
        Ok(val)
    }
}

impl Parse for NonZeroU16 {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        Ok(NonZeroU16::new(u16::parse(src)?).ok_or(ParseError::MalformedPacket)?)
    }
}

impl Parse for Bytes {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        let len = u16::parse(src)? as usize;
        ensure!(src.remaining() >= len, ParseError::InvalidLength);
        Ok(src.inner_mut().split_to(len))
    }
}

pub(crate) type ByteStr = ByteString;

impl Parse for ByteStr {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        let bytes = Bytes::parse(src)?;
        Ok(ByteString::try_from(bytes)?)
    }
}

impl Parse for (ByteStr, ByteStr) {
    fn parse<B: ByteBuf>(src: &mut B) -> Result<Self, ParseError> {
        let key = ByteStr::parse(src)?;
        let val = ByteStr::parse(src)?;
        Ok((key, val))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_decode_packet (
        ($bytes:expr, $res:expr) => {{
            let fixed = $bytes.as_ref()[0];
            let (_len, consumned) = decode_variable_length(&$bytes[1..]).unwrap().unwrap();
            let hdr = FixedHeader {
                packet_type: fixed >> 4,
                packet_flags: fixed & 0xF,
                remaining_length: $bytes.len() - consumned - 1,
            };
            let cur = Bytes::from_static(&$bytes[consumned + 1..]);
            assert_eq!(read_packet(cur, hdr), Ok($res));
        }};
    );

    fn packet_id(v: u16) -> NonZeroU16 {
        NonZeroU16::new(v).unwrap()
    }

    #[test]
    fn test_decode_variable_length() {
        macro_rules! assert_variable_length (
            ($bytes:expr, $res:expr) => {{
                assert_eq!(decode_variable_length($bytes), Ok(Some($res)));
            }};

            ($bytes:expr, $res:expr, $rest:expr) => {{
                assert_eq!(decode_variable_length($bytes), Ok(Some($res)));
            }};
        );

        assert_variable_length!(b"\x7f\x7f", (127, 1), b"\x7f");

        //assert_eq!(decode_variable_length(b"\xff\xff\xff"), Ok(None));
        assert_eq!(
            decode_variable_length(b"\xff\xff\xff\xff\xff\xff"),
            Err(ParseError::InvalidLength)
        );

        assert_variable_length!(b"\x00", (0, 1));
        assert_variable_length!(b"\x7f", (127, 1));
        assert_variable_length!(b"\x80\x01", (128, 2));
        assert_variable_length!(b"\xff\x7f", (16383, 2));
        assert_variable_length!(b"\x80\x80\x01", (16384, 3));
        assert_variable_length!(b"\xff\xff\x7f", (2097151, 3));
        assert_variable_length!(b"\x80\x80\x80\x01", (2097152, 4));
        assert_variable_length!(b"\xff\xff\xff\x7f", (268435455, 4));
    }

    // #[test]
    // fn test_decode_header() {
    //     assert_eq!(
    //         decode_header(b"\x20\x7f"),
    //         Done(
    //             &b""[..],
    //             FixedHeader {
    //                 packet_type: CONNACK,
    //                 packet_flags: 0,
    //                 remaining_length: 127,
    //             }
    //         )
    //     );

    //     assert_eq!(
    //         decode_header(b"\x3C\x82\x7f"),
    //         Done(
    //             &b""[..],
    //             FixedHeader {
    //                 packet_type: PUBLISH,
    //                 packet_flags: 0x0C,
    //                 remaining_length: 16258,
    //             }
    //         )
    //     );

    //     assert_eq!(decode_header(b"\x20"), Incomplete(Needed::Unknown));
    // }

    #[test]
    fn test_decode_connect_packets() {
        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(
                b"\x00\x04MQTT\x04\xC0\x00\x3C\x00\x0512345\x00\x04user\x00\x04pass"
            )),
            Ok(Packet::Connect(Connect {
                protocol: Protocol::MQTT(4),
                clean_session: false,
                keep_alive: 60,
                client_id: ByteString::try_from(Bytes::from_static(b"12345")).unwrap(),
                last_will: None,
                username: Some(ByteString::try_from(Bytes::from_static(b"user")).unwrap()),
                password: Some(Bytes::from(&b"pass"[..])),
            }))
        );

        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(
                b"\x00\x04MQTT\x04\x14\x00\x3C\x00\x0512345\x00\x05topic\x00\x07message"
            )),
            Ok(Packet::Connect(Connect {
                protocol: Protocol::MQTT(4),
                clean_session: false,
                keep_alive: 60,
                client_id: ByteString::try_from(Bytes::from_static(b"12345")).unwrap(),
                last_will: Some(LastWill {
                    qos: QoS::ExactlyOnce,
                    retain: false,
                    topic: ByteString::try_from(Bytes::from_static(b"topic")).unwrap(),
                    message: Bytes::from(&b"message"[..]),
                }),
                username: None,
                password: None,
            }))
        );

        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(b"\x00\x02MQ00000000000000000000")),
            Err(ParseError::InvalidProtocol),
        );
        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(b"\x00\x10MQ00000000000000000000")),
            Err(ParseError::InvalidProtocol),
        );
        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(b"\x00\x04MQAA00000000000000000000")),
            Err(ParseError::InvalidProtocol),
        );
        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(
                b"\x00\x04MQTT\x0300000000000000000000"
            )),
            Err(ParseError::UnsupportedProtocolLevel),
        );
        assert_eq!(
            decode_connect_packet(&mut Bytes::from_static(
                b"\x00\x04MQTT\x04\xff00000000000000000000"
            )),
            Err(ParseError::ConnectReservedFlagSet)
        );

        assert_eq!(
            decode_connect_ack_packet(&mut Bytes::from_static(b"\x01\x04")),
            Ok(Packet::ConnectAck {
                session_present: true,
                return_code: ConnectCode::BadUserNameOrPassword
            })
        );

        assert_eq!(
            decode_connect_ack_packet(&mut Bytes::from_static(b"\x03\x04")),
            Err(ParseError::ConnAckReservedFlagSet)
        );

        assert_decode_packet!(
            b"\x20\x02\x01\x04",
            Packet::ConnectAck {
                session_present: true,
                return_code: ConnectCode::BadUserNameOrPassword,
            }
        );

        assert_decode_packet!(b"\xe0\x00", Packet::Disconnect);
    }

    #[test]
    fn test_decode_publish_packets() {
        //assert_eq!(
        //    decode_publish_packet(b"\x00\x05topic\x12\x34"),
        //    Done(&b""[..], ("topic".to_owned(), 0x1234))
        //);

        assert_decode_packet!(
            b"\x3d\x0D\x00\x05topic\x43\x21data",
            Packet::Publish(Publish {
                dup: true,
                retain: true,
                qos: QoS::ExactlyOnce,
                topic: ByteString::try_from(Bytes::from_static(b"topic")).unwrap(),
                packet_id: Some(packet_id(0x4321)),
                payload: Bytes::from_static(b"data"),
            })
        );
        assert_decode_packet!(
            b"\x30\x0b\x00\x05topicdata",
            Packet::Publish(Publish {
                dup: false,
                retain: false,
                qos: QoS::AtMostOnce,
                topic: ByteString::try_from(Bytes::from_static(b"topic")).unwrap(),
                packet_id: None,
                payload: Bytes::from_static(b"data"),
            })
        );

        assert_decode_packet!(
            b"\x40\x02\x43\x21",
            Packet::PublishAck {
                packet_id: packet_id(0x4321),
            }
        );
        assert_decode_packet!(
            b"\x50\x02\x43\x21",
            Packet::PublishReceived {
                packet_id: packet_id(0x4321),
            }
        );
        assert_decode_packet!(
            b"\x60\x02\x43\x21",
            Packet::PublishRelease {
                packet_id: packet_id(0x4321),
            }
        );
        assert_decode_packet!(
            b"\x70\x02\x43\x21",
            Packet::PublishComplete {
                packet_id: packet_id(0x4321),
            }
        );
    }

    #[test]
    fn test_decode_subscribe_packets() {
        let p = Packet::Subscribe {
            packet_id: packet_id(0x1234),
            topic_filters: vec![
                (
                    ByteString::try_from(Bytes::from_static(b"test")).unwrap(),
                    QoS::AtLeastOnce,
                ),
                (
                    ByteString::try_from(Bytes::from_static(b"filter")).unwrap(),
                    QoS::ExactlyOnce,
                ),
            ],
        };

        assert_eq!(
            decode_subscribe_packet(&mut Bytes::from_static(
                b"\x12\x34\x00\x04test\x01\x00\x06filter\x02"
            )),
            Ok(p.clone())
        );
        assert_decode_packet!(b"\x82\x12\x12\x34\x00\x04test\x01\x00\x06filter\x02", p);

        let p = Packet::SubscribeAck {
            packet_id: packet_id(0x1234),
            status: vec![
                SubscribeReturnCode::Success(QoS::AtLeastOnce),
                SubscribeReturnCode::Failure,
                SubscribeReturnCode::Success(QoS::ExactlyOnce),
            ],
        };

        assert_eq!(
            decode_subscribe_ack_packet(&mut Bytes::from_static(b"\x12\x34\x01\x80\x02")),
            Ok(p.clone())
        );
        assert_decode_packet!(b"\x90\x05\x12\x34\x01\x80\x02", p);

        let p = Packet::Unsubscribe {
            packet_id: packet_id(0x1234),
            topic_filters: vec![
                ByteString::try_from(Bytes::from_static(b"test")).unwrap(),
                ByteString::try_from(Bytes::from_static(b"filter")).unwrap(),
            ],
        };

        assert_eq!(
            decode_unsubscribe_packet(&mut Bytes::from_static(
                b"\x12\x34\x00\x04test\x00\x06filter"
            )),
            Ok(p.clone())
        );
        assert_decode_packet!(b"\xa2\x10\x12\x34\x00\x04test\x00\x06filter", p);

        assert_decode_packet!(
            b"\xb0\x02\x43\x21",
            Packet::UnsubscribeAck {
                packet_id: packet_id(0x4321),
            }
        );
    }

    #[test]
    fn test_decode_ping_packets() {
        assert_decode_packet!(b"\xc0\x00", Packet::PingRequest);
        assert_decode_packet!(b"\xd0\x00", Packet::PingResponse);
    }
}
