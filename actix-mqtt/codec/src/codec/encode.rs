use bytes::{BufMut, BytesMut};

use crate::packet::*;
use crate::proto::*;

use super::{ConnectFlags, WILL_QOS_SHIFT};

pub fn write_packet(packet: &Packet, dst: &mut BytesMut, content_size: usize) {
    write_fixed_header(packet, dst, content_size);
    write_content(packet, dst);
}

pub fn get_encoded_size(packet: &Packet) -> usize {
    match *packet {
        Packet::Connect ( ref connect ) => {
            match *connect {
                Connect {ref last_will, ref client_id, ref username, ref password, ..} =>
                {
                    // Protocol Name + Protocol Level + Connect Flags + Keep Alive
                    let mut n = 2 + 4 + 1 + 1 + 2;

                    // Client Id
                    n += 2 + client_id.len();

                    // Will Topic + Will Message
                    if let Some(LastWill { ref topic, ref message, .. }) = *last_will {
                        n += 2 + topic.len() + 2 + message.len();
                    }

                    if let Some(ref s) = *username {
                        n += 2 + s.len();
                    }

                    if let Some(ref s) = *password {
                        n += 2 + s.len();
                    }

                    n
                }
            }
        }

        Packet::Publish( Publish{ qos, ref topic, ref payload, .. }) => {
            // Topic + Packet Id + Payload
            if qos == QoS::AtLeastOnce || qos == QoS::ExactlyOnce {
                4 + topic.len() + payload.len()
            } else {
                2 + topic.len() + payload.len()
            }
        }

        Packet::ConnectAck { .. } | // Flags + Return Code
        Packet::PublishAck { .. } | // Packet Id
        Packet::PublishReceived { .. } | // Packet Id
        Packet::PublishRelease { .. } | // Packet Id
        Packet::PublishComplete { .. } | // Packet Id
        Packet::UnsubscribeAck { .. } => 2, // Packet Id

        Packet::Subscribe { ref topic_filters, .. } => {
            2 + topic_filters.iter().fold(0, |acc, &(ref filter, _)| acc + 2 + filter.len() + 1)
        }

        Packet::SubscribeAck { ref status, .. } => 2 + status.len(),

        Packet::Unsubscribe { ref topic_filters, .. } => {
            2 + topic_filters.iter().fold(0, |acc, filter| acc + 2 + filter.len())
        }

        Packet::PingRequest | Packet::PingResponse | Packet::Disconnect => 0,
    }
}

#[inline]
fn write_fixed_header(packet: &Packet, dst: &mut BytesMut, content_size: usize) {
    dst.put_u8((packet.packet_type() << 4) | packet.packet_flags());
    write_variable_length(content_size, dst);
}

fn write_content(packet: &Packet, dst: &mut BytesMut) {
    match *packet {
        Packet::Connect(ref connect) => match *connect {
            Connect {
                protocol,
                clean_session,
                keep_alive,
                ref last_will,
                ref client_id,
                ref username,
                ref password,
            } => {
                write_slice(protocol.name().as_bytes(), dst);

                let mut flags = ConnectFlags::empty();

                if username.is_some() {
                    flags |= ConnectFlags::USERNAME;
                }
                if password.is_some() {
                    flags |= ConnectFlags::PASSWORD;
                }

                if let Some(LastWill { qos, retain, .. }) = *last_will {
                    flags |= ConnectFlags::WILL;

                    if retain {
                        flags |= ConnectFlags::WILL_RETAIN;
                    }

                    let b: u8 = qos as u8;

                    flags |= ConnectFlags::from_bits_truncate(b << WILL_QOS_SHIFT);
                }

                if clean_session {
                    flags |= ConnectFlags::CLEAN_SESSION;
                }

                dst.put_slice(&[protocol.level(), flags.bits()]);

                dst.put_u16(keep_alive);

                write_slice(client_id.as_bytes(), dst);

                if let Some(LastWill {
                    ref topic,
                    ref message,
                    ..
                }) = *last_will
                {
                    write_slice(topic.as_bytes(), dst);
                    write_slice(&message, dst);
                }

                if let Some(ref s) = *username {
                    write_slice(s.as_bytes(), dst);
                }

                if let Some(ref s) = *password {
                    write_slice(s, dst);
                }
            }
        },

        Packet::ConnectAck {
            session_present,
            return_code,
        } => {
            dst.put_slice(&[if session_present { 0x01 } else { 0x00 }, return_code as u8]);
        }

        Packet::Publish(Publish {
            qos,
            ref topic,
            packet_id,
            ref payload,
            ..
        }) => {
            write_slice(topic.as_bytes(), dst);

            if qos == QoS::AtLeastOnce || qos == QoS::ExactlyOnce {
                dst.put_u16(packet_id.unwrap().into());
            }

            dst.put(payload.as_ref());
        }

        Packet::PublishAck { packet_id }
        | Packet::PublishReceived { packet_id }
        | Packet::PublishRelease { packet_id }
        | Packet::PublishComplete { packet_id }
        | Packet::UnsubscribeAck { packet_id } => {
            dst.put_u16(packet_id.into());
        }

        Packet::Subscribe {
            packet_id,
            ref topic_filters,
        } => {
            dst.put_u16(packet_id.into());

            for &(ref filter, qos) in topic_filters {
                write_slice(filter.as_ref(), dst);
                dst.put_slice(&[qos as u8]);
            }
        }

        Packet::SubscribeAck {
            packet_id,
            ref status,
        } => {
            dst.put_u16(packet_id.into());

            let buf: Vec<u8> = status
                .iter()
                .map(|s| {
                    if let SubscribeReturnCode::Success(qos) = *s {
                        qos as u8
                    } else {
                        0x80
                    }
                })
                .collect();

            dst.put_slice(&buf);
        }

        Packet::Unsubscribe {
            packet_id,
            ref topic_filters,
        } => {
            dst.put_u16(packet_id.into());

            for filter in topic_filters {
                write_slice(filter.as_ref(), dst);
            }
        }

        Packet::PingRequest | Packet::PingResponse | Packet::Disconnect => {}
    }
}

#[inline]
fn write_slice(r: &[u8], dst: &mut BytesMut) {
    dst.put_u16(r.len() as u16);
    dst.put_slice(r);
}

#[inline]
fn write_variable_length(size: usize, dst: &mut BytesMut) {
    // todo: verify at higher level
    // if size > MAX_VARIABLE_LENGTH {
    //     Err(Error::new(ErrorKind::Other, "out of range"))
    if size <= 127 {
        dst.put_u8(size as u8);
    } else if size <= 16383 {
        // 127 + 127 << 7
        dst.put_slice(&[((size % 128) | 0x80) as u8, (size >> 7) as u8]);
    } else if size <= 2_097_151 {
        // 127 + 127 << 7 + 127 << 14
        dst.put_slice(&[
            ((size % 128) | 0x80) as u8,
            (((size >> 7) % 128) | 0x80) as u8,
            (size >> 14) as u8,
        ]);
    } else {
        dst.put_slice(&[
            ((size % 128) | 0x80) as u8,
            (((size >> 7) % 128) | 0x80) as u8,
            (((size >> 14) % 128) | 0x80) as u8,
            (size >> 21) as u8,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use bytestring::ByteString;
    use std::num::NonZeroU16;

    use super::*;

    fn packet_id(v: u16) -> NonZeroU16 {
        NonZeroU16::new(v).unwrap()
    }

    #[test]
    fn test_encode_variable_length() {
        let mut v = BytesMut::new();

        write_variable_length(123, &mut v);
        assert_eq!(v, [123].as_ref());

        v.clear();

        write_variable_length(129, &mut v);
        assert_eq!(v, b"\x81\x01".as_ref());

        v.clear();

        write_variable_length(16383, &mut v);
        assert_eq!(v, b"\xff\x7f".as_ref());

        v.clear();

        write_variable_length(2097151, &mut v);
        assert_eq!(v, b"\xff\xff\x7f".as_ref());

        v.clear();

        write_variable_length(268435455, &mut v);
        assert_eq!(v, b"\xff\xff\xff\x7f".as_ref());

        // assert!(v.write_variable_length(MAX_VARIABLE_LENGTH + 1).is_err())
    }

    #[test]
    fn test_encode_fixed_header() {
        let mut v = BytesMut::new();
        let p = Packet::PingRequest;

        assert_eq!(get_encoded_size(&p), 0);
        write_fixed_header(&p, &mut v, 0);
        assert_eq!(v, b"\xc0\x00".as_ref());

        v.clear();

        let p = Packet::Publish(Publish {
            dup: true,
            retain: true,
            qos: QoS::ExactlyOnce,
            topic: ByteString::from_static("topic"),
            packet_id: Some(packet_id(0x4321)),
            payload: (0..255).collect::<Vec<u8>>().into(),
        });

        assert_eq!(get_encoded_size(&p), 264);
        write_fixed_header(&p, &mut v, 264);
        assert_eq!(v, b"\x3d\x88\x02".as_ref());
    }

    macro_rules! assert_packet {
        ($p:expr, $data:expr) => {
            let mut v = BytesMut::with_capacity(1024);
            write_packet(&$p, &mut v, get_encoded_size($p));
            assert_eq!(v.len(), $data.len());
            assert_eq!(v, &$data[..]);
            // assert_eq!(read_packet($data.cursor()).unwrap(), (&b""[..], $p));
        };
    }

    #[test]
    fn test_encode_connect_packets() {
        assert_packet!(
            &Packet::Connect(Connect {
                protocol: Protocol::MQTT(4),
                clean_session: false,
                keep_alive: 60,
                client_id: ByteString::from_static("12345"),
                last_will: None,
                username: Some(ByteString::from_static("user")),
                password: Some(Bytes::from_static(b"pass")),
            }),
            &b"\x10\x1D\x00\x04MQTT\x04\xC0\x00\x3C\x00\
\x0512345\x00\x04user\x00\x04pass"[..]
        );

        assert_packet!(
            &Packet::Connect(Connect {
                protocol: Protocol::MQTT(4),
                clean_session: false,
                keep_alive: 60,
                client_id: ByteString::from_static("12345"),
                last_will: Some(LastWill {
                    qos: QoS::ExactlyOnce,
                    retain: false,
                    topic: ByteString::from_static("topic"),
                    message: Bytes::from_static(b"message"),
                }),
                username: None,
                password: None,
            }),
            &b"\x10\x21\x00\x04MQTT\x04\x14\x00\x3C\x00\
\x0512345\x00\x05topic\x00\x07message"[..]
        );

        assert_packet!(&Packet::Disconnect, b"\xe0\x00");
    }

    #[test]
    fn test_encode_publish_packets() {
        assert_packet!(
            &Packet::Publish(Publish {
                dup: true,
                retain: true,
                qos: QoS::ExactlyOnce,
                topic: ByteString::from_static("topic"),
                packet_id: Some(packet_id(0x4321)),
                payload: Bytes::from_static(b"data"),
            }),
            b"\x3d\x0D\x00\x05topic\x43\x21data"
        );

        assert_packet!(
            &Packet::Publish(Publish {
                dup: false,
                retain: false,
                qos: QoS::AtMostOnce,
                topic: ByteString::from_static("topic"),
                packet_id: None,
                payload: Bytes::from_static(b"data"),
            }),
            b"\x30\x0b\x00\x05topicdata"
        );
    }

    #[test]
    fn test_encode_subscribe_packets() {
        assert_packet!(
            &Packet::Subscribe {
                packet_id: packet_id(0x1234),
                topic_filters: vec![
                    (ByteString::from_static("test"), QoS::AtLeastOnce),
                    (ByteString::from_static("filter"), QoS::ExactlyOnce)
                ],
            },
            b"\x82\x12\x12\x34\x00\x04test\x01\x00\x06filter\x02"
        );

        assert_packet!(
            &Packet::SubscribeAck {
                packet_id: packet_id(0x1234),
                status: vec![
                    SubscribeReturnCode::Success(QoS::AtLeastOnce),
                    SubscribeReturnCode::Failure,
                    SubscribeReturnCode::Success(QoS::ExactlyOnce)
                ],
            },
            b"\x90\x05\x12\x34\x01\x80\x02"
        );

        assert_packet!(
            &Packet::Unsubscribe {
                packet_id: packet_id(0x1234),
                topic_filters: vec![
                    ByteString::from_static("test"),
                    ByteString::from_static("filter"),
                ],
            },
            b"\xa2\x10\x12\x34\x00\x04test\x00\x06filter"
        );

        assert_packet!(
            &Packet::UnsubscribeAck {
                packet_id: packet_id(0x4321)
            },
            b"\xb0\x02\x43\x21"
        );
    }

    #[test]
    fn test_encode_ping_packets() {
        assert_packet!(&Packet::PingRequest, b"\xc0\x00");
        assert_packet!(&Packet::PingResponse, b"\xd0\x00");
    }
}
