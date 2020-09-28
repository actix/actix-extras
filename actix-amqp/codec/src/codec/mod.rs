use bytes::BytesMut;
use std::marker::Sized;

use crate::errors::AmqpParseError;

macro_rules! decode_check_len {
    ($buf:ident, $size:expr) => {
        if $buf.len() < $size {
            return Err(AmqpParseError::Incomplete(Some($size)));
        }
    };
}

#[macro_use]
mod decode;
mod encode;

pub(crate) use self::decode::decode_list_header;

pub trait Encode {
    fn encoded_size(&self) -> usize;

    fn encode(&self, buf: &mut BytesMut);
}

pub trait ArrayEncode {
    const ARRAY_FORMAT_CODE: u8;

    fn array_encoded_size(&self) -> usize;

    fn array_encode(&self, buf: &mut BytesMut);
}

pub trait Decode
where
    Self: Sized,
{
    fn decode(input: &[u8]) -> Result<(&[u8], Self), AmqpParseError>;
}

pub trait DecodeFormatted
where
    Self: Sized,
{
    fn decode_with_format(input: &[u8], fmt: u8) -> Result<(&[u8], Self), AmqpParseError>;
}

pub trait ArrayDecode: Sized {
    fn array_decode(input: &[u8]) -> Result<(&[u8], Self), AmqpParseError>;
}

impl<T: DecodeFormatted> Decode for T {
    fn decode(input: &[u8]) -> Result<(&[u8], Self), AmqpParseError> {
        let (input, fmt) = decode_format_code(input)?;
        T::decode_with_format(input, fmt)
    }
}

pub fn decode_format_code(input: &[u8]) -> Result<(&[u8], u8), AmqpParseError> {
    decode_check_len!(input, 1);
    Ok((&input[1..], input[0]))
}

pub const FORMATCODE_DESCRIBED: u8 = 0x00;
pub const FORMATCODE_NULL: u8 = 0x40; // fixed width --V
pub const FORMATCODE_BOOLEAN: u8 = 0x56;
pub const FORMATCODE_BOOLEAN_TRUE: u8 = 0x41;
pub const FORMATCODE_BOOLEAN_FALSE: u8 = 0x42;
pub const FORMATCODE_UINT_0: u8 = 0x43;
pub const FORMATCODE_ULONG_0: u8 = 0x44;
pub const FORMATCODE_UBYTE: u8 = 0x50;
pub const FORMATCODE_USHORT: u8 = 0x60;
pub const FORMATCODE_UINT: u8 = 0x70;
pub const FORMATCODE_ULONG: u8 = 0x80;
pub const FORMATCODE_BYTE: u8 = 0x51;
pub const FORMATCODE_SHORT: u8 = 0x61;
pub const FORMATCODE_INT: u8 = 0x71;
pub const FORMATCODE_LONG: u8 = 0x81;
pub const FORMATCODE_SMALLUINT: u8 = 0x52;
pub const FORMATCODE_SMALLULONG: u8 = 0x53;
pub const FORMATCODE_SMALLINT: u8 = 0x54;
pub const FORMATCODE_SMALLLONG: u8 = 0x55;
pub const FORMATCODE_FLOAT: u8 = 0x72;
pub const FORMATCODE_DOUBLE: u8 = 0x82;
// pub const FORMATCODE_DECIMAL32: u8 = 0x74;
// pub const FORMATCODE_DECIMAL64: u8 = 0x84;
// pub const FORMATCODE_DECIMAL128: u8 = 0x94;
pub const FORMATCODE_CHAR: u8 = 0x73;
pub const FORMATCODE_TIMESTAMP: u8 = 0x83;
pub const FORMATCODE_UUID: u8 = 0x98;
pub const FORMATCODE_BINARY8: u8 = 0xa0; // variable --V
pub const FORMATCODE_BINARY32: u8 = 0xb0;
pub const FORMATCODE_STRING8: u8 = 0xa1;
pub const FORMATCODE_STRING32: u8 = 0xb1;
pub const FORMATCODE_SYMBOL8: u8 = 0xa3;
pub const FORMATCODE_SYMBOL32: u8 = 0xb3;
pub const FORMATCODE_LIST0: u8 = 0x45; // compound --V
pub const FORMATCODE_LIST8: u8 = 0xc0;
pub const FORMATCODE_LIST32: u8 = 0xd0;
pub const FORMATCODE_MAP8: u8 = 0xc1;
pub const FORMATCODE_MAP32: u8 = 0xd1;
pub const FORMATCODE_ARRAY8: u8 = 0xe0;
pub const FORMATCODE_ARRAY32: u8 = 0xf0;

#[cfg(test)]
mod tests {
    use bytes::{Bytes, BytesMut};

    use crate::codec::{Decode, Encode};
    use crate::errors::AmqpCodecError;
    use crate::framing::{AmqpFrame, SaslFrame};
    use crate::protocol::SaslFrameBody;

    #[test]
    fn test_sasl_mechanisms() -> Result<(), AmqpCodecError> {
        let data = b"\x02\x01\0\0\0S@\xc02\x01\xe0/\x04\xb3\0\0\0\x07MSSBCBS\0\0\0\x05PLAIN\0\0\0\tANONYMOUS\0\0\0\x08EXTERNAL";

        let (remainder, frame) = SaslFrame::decode(data.as_ref())?;
        assert!(remainder.is_empty());
        match frame.body {
            SaslFrameBody::SaslMechanisms(_) => (),
            _ => panic!("error"),
        }

        let mut buf = BytesMut::new();
        buf.reserve(frame.encoded_size());
        frame.encode(&mut buf);
        buf.split_to(4);
        assert_eq!(Bytes::from_static(data), buf.freeze());

        Ok(())
    }

    #[test]
    fn test_disposition() -> Result<(), AmqpCodecError> {
        let data = b"\x02\0\0\0\0S\x15\xc0\x0c\x06AC@A\0S$\xc0\x01\0B";

        let (remainder, frame) = AmqpFrame::decode(data.as_ref())?;
        assert!(remainder.is_empty());
        assert_eq!(frame.performative().name(), "Disposition");

        let mut buf = BytesMut::new();
        buf.reserve(frame.encoded_size());
        frame.encode(&mut buf);
        buf.split_to(4);
        assert_eq!(Bytes::from_static(data), buf.freeze());

        Ok(())
    }
}
