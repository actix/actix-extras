use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::{Encode, FORMATCODE_BINARY32, FORMATCODE_BINARY8};
use crate::protocol::TransferBody;
use crate::types::{Descriptor, List, Variant};

use super::SECTION_PREFIX_LENGTH;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageBody {
    pub data: Vec<Bytes>,
    pub sequence: Vec<List>,
    pub messages: Vec<TransferBody>,
    pub value: Option<Variant>,
}

impl MessageBody {
    pub fn data(&self) -> Option<&Bytes> {
        if self.data.is_empty() {
            None
        } else {
            Some(&self.data[0])
        }
    }

    pub fn value(&self) -> Option<&Variant> {
        self.value.as_ref()
    }

    pub fn set_data(&mut self, data: Bytes) {
        self.data.clear();
        self.data.push(data);
    }
}

impl Encode for MessageBody {
    fn encoded_size(&self) -> usize {
        let mut size = self
            .data
            .iter()
            .fold(0, |a, d| a + d.encoded_size() + SECTION_PREFIX_LENGTH);
        size += self
            .sequence
            .iter()
            .fold(0, |a, seq| a + seq.encoded_size() + SECTION_PREFIX_LENGTH);
        size += self.messages.iter().fold(0, |a, m| {
            let length = m.encoded_size();
            let size = length + if length > std::u8::MAX as usize { 5 } else { 2 };
            a + size + SECTION_PREFIX_LENGTH
        });

        if let Some(ref val) = self.value {
            size + val.encoded_size() + SECTION_PREFIX_LENGTH
        } else {
            size
        }
    }

    fn encode(&self, dst: &mut BytesMut) {
        self.data.iter().for_each(|d| {
            Descriptor::Ulong(117).encode(dst);
            d.encode(dst);
        });
        self.sequence.iter().for_each(|seq| {
            Descriptor::Ulong(118).encode(dst);
            seq.encode(dst)
        });
        if let Some(ref val) = self.value {
            Descriptor::Ulong(119).encode(dst);
            val.encode(dst);
        }
        // encode Message as nested Bytes object
        self.messages.iter().for_each(|m| {
            Descriptor::Ulong(117).encode(dst);

            // Bytes prefix
            let length = m.encoded_size();
            if length > std::u8::MAX as usize {
                dst.put_u8(FORMATCODE_BINARY32);
                dst.put_u32(length as u32);
            } else {
                dst.put_u8(FORMATCODE_BINARY8);
                dst.put_u8(length as u8);
            }
            // encode nested Message
            m.encode(dst);
        });
    }
}
