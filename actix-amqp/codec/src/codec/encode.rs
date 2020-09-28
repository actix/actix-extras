use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};
use std::{i8, u8};

use bytes::{BufMut, Bytes, BytesMut};
use bytestring::ByteString;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::codec::{self, ArrayEncode, Encode};
use crate::framing::{self, AmqpFrame, SaslFrame};
use crate::types::{
    Descriptor, List, Multiple, StaticSymbol, Str, Symbol, Variant, VecStringMap, VecSymbolMap,
};

fn encode_null(buf: &mut BytesMut) {
    buf.put_u8(codec::FORMATCODE_NULL);
}

pub trait FixedEncode {}

impl<T: FixedEncode + ArrayEncode> Encode for T {
    fn encoded_size(&self) -> usize {
        self.array_encoded_size() + 1
    }
    fn encode(&self, buf: &mut BytesMut) {
        buf.put_u8(T::ARRAY_FORMAT_CODE);
        self.array_encode(buf);
    }
}

impl Encode for bool {
    fn encoded_size(&self) -> usize {
        1
    }
    fn encode(&self, buf: &mut BytesMut) {
        buf.put_u8(if *self {
            codec::FORMATCODE_BOOLEAN_TRUE
        } else {
            codec::FORMATCODE_BOOLEAN_FALSE
        });
    }
}
impl ArrayEncode for bool {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_BOOLEAN;
    fn array_encoded_size(&self) -> usize {
        1
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u8(if *self { 1 } else { 0 });
    }
}

impl FixedEncode for u8 {}
impl ArrayEncode for u8 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_UBYTE;
    fn array_encoded_size(&self) -> usize {
        1
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u8(*self);
    }
}

impl FixedEncode for u16 {}
impl ArrayEncode for u16 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_USHORT;
    fn array_encoded_size(&self) -> usize {
        2
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u16(*self);
    }
}

impl Encode for u32 {
    fn encoded_size(&self) -> usize {
        if *self == 0 {
            1
        } else if *self > u32::from(u8::MAX) {
            5
        } else {
            2
        }
    }
    fn encode(&self, buf: &mut BytesMut) {
        if *self == 0 {
            buf.put_u8(codec::FORMATCODE_UINT_0)
        } else if *self > u32::from(u8::MAX) {
            buf.put_u8(codec::FORMATCODE_UINT);
            buf.put_u32(*self);
        } else {
            buf.put_u8(codec::FORMATCODE_SMALLUINT);
            buf.put_u8(*self as u8);
        }
    }
}
impl ArrayEncode for u32 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_UINT;
    fn array_encoded_size(&self) -> usize {
        4
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u32(*self);
    }
}

impl Encode for u64 {
    fn encoded_size(&self) -> usize {
        if *self == 0 {
            1
        } else if *self > u64::from(u8::MAX) {
            9
        } else {
            2
        }
    }

    fn encode(&self, buf: &mut BytesMut) {
        if *self == 0 {
            buf.put_u8(codec::FORMATCODE_ULONG_0)
        } else if *self > u64::from(u8::MAX) {
            buf.put_u8(codec::FORMATCODE_ULONG);
            buf.put_u64(*self);
        } else {
            buf.put_u8(codec::FORMATCODE_SMALLULONG);
            buf.put_u8(*self as u8);
        }
    }
}

impl ArrayEncode for u64 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_ULONG;
    fn array_encoded_size(&self) -> usize {
        8
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u64(*self);
    }
}

impl FixedEncode for i8 {}

impl ArrayEncode for i8 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_BYTE;
    fn array_encoded_size(&self) -> usize {
        1
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_i8(*self);
    }
}

impl FixedEncode for i16 {}

impl ArrayEncode for i16 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_SHORT;
    fn array_encoded_size(&self) -> usize {
        2
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_i16(*self);
    }
}

impl Encode for i32 {
    fn encoded_size(&self) -> usize {
        if *self > i32::from(i8::MAX) || *self < i32::from(i8::MIN) {
            5
        } else {
            2
        }
    }

    fn encode(&self, buf: &mut BytesMut) {
        if *self > i32::from(i8::MAX) || *self < i32::from(i8::MIN) {
            buf.put_u8(codec::FORMATCODE_INT);
            buf.put_i32(*self);
        } else {
            buf.put_u8(codec::FORMATCODE_SMALLINT);
            buf.put_i8(*self as i8);
        }
    }
}

impl ArrayEncode for i32 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_INT;

    fn array_encoded_size(&self) -> usize {
        4
    }

    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_i32(*self);
    }
}

impl Encode for i64 {
    fn encoded_size(&self) -> usize {
        if *self > i64::from(i8::MAX) || *self < i64::from(i8::MIN) {
            9
        } else {
            2
        }
    }

    fn encode(&self, buf: &mut BytesMut) {
        if *self > i64::from(i8::MAX) || *self < i64::from(i8::MIN) {
            buf.put_u8(codec::FORMATCODE_LONG);
            buf.put_i64(*self);
        } else {
            buf.put_u8(codec::FORMATCODE_SMALLLONG);
            buf.put_i8(*self as i8);
        }
    }
}

impl ArrayEncode for i64 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_LONG;
    fn array_encoded_size(&self) -> usize {
        8
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_i64(*self);
    }
}

impl FixedEncode for f32 {}

impl ArrayEncode for f32 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_FLOAT;

    fn array_encoded_size(&self) -> usize {
        4
    }

    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_f32(*self);
    }
}

impl FixedEncode for f64 {}

impl ArrayEncode for f64 {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_DOUBLE;
    fn array_encoded_size(&self) -> usize {
        8
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_f64(*self);
    }
}

impl FixedEncode for char {}

impl ArrayEncode for char {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_CHAR;
    fn array_encoded_size(&self) -> usize {
        4
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u32(*self as u32);
    }
}

impl FixedEncode for DateTime<Utc> {}

impl ArrayEncode for DateTime<Utc> {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_TIMESTAMP;
    fn array_encoded_size(&self) -> usize {
        8
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        let timestamp = self.timestamp() * 1000 + i64::from(self.timestamp_subsec_millis());
        buf.put_i64(timestamp);
    }
}

impl FixedEncode for Uuid {}

impl ArrayEncode for Uuid {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_UUID;
    fn array_encoded_size(&self) -> usize {
        16
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.extend_from_slice(self.as_bytes());
    }
}

impl Encode for Bytes {
    fn encoded_size(&self) -> usize {
        let length = self.len();
        let size = if length > u8::MAX as usize { 5 } else { 2 };
        size + length
    }

    fn encode(&self, buf: &mut BytesMut) {
        let length = self.len();
        if length > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_BINARY32);
            buf.put_u32(length as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_BINARY8);
            buf.put_u8(length as u8);
        }
        buf.put_slice(self);
    }
}

impl ArrayEncode for Bytes {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_BINARY32;
    fn array_encoded_size(&self) -> usize {
        4 + self.len()
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.len() as u32);
        buf.put_slice(&self);
    }
}

impl Encode for ByteString {
    fn encoded_size(&self) -> usize {
        let length = self.len();
        let size = if length > u8::MAX as usize { 5 } else { 2 };
        size + length
    }

    fn encode(&self, buf: &mut BytesMut) {
        let length = self.len();
        if length > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_STRING32);
            buf.put_u32(length as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_STRING8);
            buf.put_u8(length as u8);
        }
        buf.put_slice(self.as_bytes());
    }
}
impl ArrayEncode for ByteString {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_STRING32;
    fn array_encoded_size(&self) -> usize {
        4 + self.len()
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.len() as u32);
        buf.put_slice(self.as_bytes());
    }
}

impl Encode for str {
    fn encoded_size(&self) -> usize {
        let length = self.len();
        let size = if length > u8::MAX as usize { 5 } else { 2 };
        size + length
    }

    fn encode(&self, buf: &mut BytesMut) {
        let length = self.len();
        if length > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_STRING32);
            buf.put_u32(length as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_STRING8);
            buf.put_u8(length as u8);
        }
        buf.put_slice(self.as_bytes());
    }
}

impl ArrayEncode for str {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_STRING32;
    fn array_encoded_size(&self) -> usize {
        4 + self.len()
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.len() as u32);
        buf.put_slice(self.as_bytes());
    }
}

impl Encode for Str {
    fn encoded_size(&self) -> usize {
        let length = self.len();
        let size = if length > u8::MAX as usize { 5 } else { 2 };
        size + length
    }

    fn encode(&self, buf: &mut BytesMut) {
        let length = self.as_str().len();
        if length > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_STRING32);
            buf.put_u32(length as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_STRING8);
            buf.put_u8(length as u8);
        }
        buf.put_slice(self.as_bytes());
    }
}

impl Encode for Symbol {
    fn encoded_size(&self) -> usize {
        let length = self.len();
        let size = if length > u8::MAX as usize { 5 } else { 2 };
        size + length
    }

    fn encode(&self, buf: &mut BytesMut) {
        let length = self.as_str().len();
        if length > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_SYMBOL32);
            buf.put_u32(length as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_SYMBOL8);
            buf.put_u8(length as u8);
        }
        buf.put_slice(self.as_bytes());
    }
}

impl ArrayEncode for Symbol {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_SYMBOL32;
    fn array_encoded_size(&self) -> usize {
        4 + self.len()
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.len() as u32);
        buf.put_slice(self.as_bytes());
    }
}

impl Encode for StaticSymbol {
    fn encoded_size(&self) -> usize {
        let length = self.0.len();
        let size = if length > u8::MAX as usize { 5 } else { 2 };
        size + length
    }

    fn encode(&self, buf: &mut BytesMut) {
        let length = self.0.len();
        if length > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_SYMBOL32);
            buf.put_u32(length as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_SYMBOL8);
            buf.put_u8(length as u8);
        }
        buf.put_slice(self.0.as_bytes());
    }
}

fn map_encoded_size<K: Hash + Eq + Encode, V: Encode, S: BuildHasher>(
    map: &HashMap<K, V, S>,
) -> usize {
    map.iter()
        .fold(0, |r, (k, v)| r + k.encoded_size() + v.encoded_size())
}
impl<K: Eq + Hash + Encode, V: Encode, S: BuildHasher> Encode for HashMap<K, V, S> {
    fn encoded_size(&self) -> usize {
        let size = map_encoded_size(self);
        // f:1 + s:4 + c:4 vs f:1 + s:1 + c:1
        let preamble = if size + 1 > u8::MAX as usize { 9 } else { 3 };
        preamble + size
    }

    fn encode(&self, buf: &mut BytesMut) {
        let count = self.len() * 2; // key-value pair accounts for two items in count
        let size = map_encoded_size(self);
        if size + 1 > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_MAP32);
            buf.put_u32((size + 4) as u32); // +4 for 4 byte count that follows
            buf.put_u32(count as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_MAP8);
            buf.put_u8((size + 1) as u8); // +1 for 1 byte count that follows
            buf.put_u8(count as u8);
        }

        for (k, v) in self {
            k.encode(buf);
            v.encode(buf);
        }
    }
}

impl<K: Eq + Hash + Encode, V: Encode> ArrayEncode for HashMap<K, V> {
    const ARRAY_FORMAT_CODE: u8 = codec::FORMATCODE_MAP32;
    fn array_encoded_size(&self) -> usize {
        8 + map_encoded_size(self)
    }
    fn array_encode(&self, buf: &mut BytesMut) {
        let count = self.len() * 2;
        let size = map_encoded_size(self) + 4;
        buf.put_u32(size as u32);
        buf.put_u32(count as u32);

        for (k, v) in self {
            k.encode(buf);
            v.encode(buf);
        }
    }
}

impl Encode for VecSymbolMap {
    fn encoded_size(&self) -> usize {
        let size = self
            .0
            .iter()
            .fold(0, |r, (k, v)| r + k.encoded_size() + v.encoded_size());

        // f:1 + s:4 + c:4 vs f:1 + s:1 + c:1
        let preamble = if size + 1 > u8::MAX as usize { 9 } else { 3 };
        preamble + size
    }

    fn encode(&self, buf: &mut BytesMut) {
        let count = self.len() * 2; // key-value pair accounts for two items in count
        let size = self
            .0
            .iter()
            .fold(0, |r, (k, v)| r + k.encoded_size() + v.encoded_size());

        if size + 1 > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_MAP32);
            buf.put_u32((size + 4) as u32); // +4 for 4 byte count that follows
            buf.put_u32(count as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_MAP8);
            buf.put_u8((size + 1) as u8); // +1 for 1 byte count that follows
            buf.put_u8(count as u8);
        }

        for (k, v) in self.iter() {
            k.encode(buf);
            v.encode(buf);
        }
    }
}

impl Encode for VecStringMap {
    fn encoded_size(&self) -> usize {
        let size = self
            .0
            .iter()
            .fold(0, |r, (k, v)| r + k.encoded_size() + v.encoded_size());

        // f:1 + s:4 + c:4 vs f:1 + s:1 + c:1
        let preamble = if size + 1 > u8::MAX as usize { 9 } else { 3 };
        preamble + size
    }

    fn encode(&self, buf: &mut BytesMut) {
        let count = self.len() * 2; // key-value pair accounts for two items in count
        let size = self
            .0
            .iter()
            .fold(0, |r, (k, v)| r + k.encoded_size() + v.encoded_size());

        if size + 1 > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_MAP32);
            buf.put_u32((size + 4) as u32); // +4 for 4 byte count that follows
            buf.put_u32(count as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_MAP8);
            buf.put_u8((size + 1) as u8); // +1 for 1 byte count that follows
            buf.put_u8(count as u8);
        }

        for (k, v) in self.iter() {
            k.encode(buf);
            v.encode(buf);
        }
    }
}

fn array_encoded_size<T: ArrayEncode>(vec: &[T]) -> usize {
    vec.iter().fold(0, |r, i| r + i.array_encoded_size())
}

impl<T: ArrayEncode> Encode for Vec<T> {
    fn encoded_size(&self) -> usize {
        let content_size = array_encoded_size(self);
        // format_code + size + count + item constructor -- todo: support described ctor?
        (if content_size + 1 > u8::MAX as usize {
            10
        } else {
            4
        }) // +1 for 1 byte count and 1 byte format code
            + content_size
    }

    fn encode(&self, buf: &mut BytesMut) {
        let size = array_encoded_size(self);
        if size + 1 > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_ARRAY32);
            buf.put_u32((size + 5) as u32); // +4 for 4 byte count and 1 byte item ctor that follow
            buf.put_u32(self.len() as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_ARRAY8);
            buf.put_u8((size + 2) as u8); // +1 for 1 byte count and 1 byte item ctor that follow
            buf.put_u8(self.len() as u8);
        }
        buf.put_u8(T::ARRAY_FORMAT_CODE);
        for i in self {
            i.array_encode(buf);
        }
    }
}

impl<T: Encode + ArrayEncode> Encode for Multiple<T> {
    fn encoded_size(&self) -> usize {
        let count = self.len();
        if count == 1 {
            // special case: single item is encoded without array encoding
            self.0[0].encoded_size()
        } else {
            self.0.encoded_size()
        }
    }

    fn encode(&self, buf: &mut BytesMut) {
        let count = self.0.len();
        if count == 1 {
            // special case: single item is encoded without array encoding
            self.0[0].encode(buf)
        } else {
            self.0.encode(buf)
        }
    }
}

fn list_encoded_size(vec: &List) -> usize {
    vec.iter().fold(0, |r, i| r + i.encoded_size())
}

impl Encode for List {
    fn encoded_size(&self) -> usize {
        let content_size = list_encoded_size(self);
        // format_code + size + count
        (if content_size + 1 > u8::MAX as usize {
            9
        } else {
            3
        }) + content_size
    }

    fn encode(&self, buf: &mut BytesMut) {
        let size = list_encoded_size(self);
        if size + 1 > u8::MAX as usize {
            buf.put_u8(codec::FORMATCODE_ARRAY32);
            buf.put_u32((size + 4) as u32); // +4 for 4 byte count that follow
            buf.put_u32(self.len() as u32);
        } else {
            buf.put_u8(codec::FORMATCODE_ARRAY8);
            buf.put_u8((size + 1) as u8); // +1 for 1 byte count that follow
            buf.put_u8(self.len() as u8);
        }
        for i in self.iter() {
            i.encode(buf);
        }
    }
}

impl Encode for Variant {
    fn encoded_size(&self) -> usize {
        match *self {
            Variant::Null => 1,
            Variant::Boolean(b) => b.encoded_size(),
            Variant::Ubyte(b) => b.encoded_size(),
            Variant::Ushort(s) => s.encoded_size(),
            Variant::Uint(i) => i.encoded_size(),
            Variant::Ulong(l) => l.encoded_size(),
            Variant::Byte(b) => b.encoded_size(),
            Variant::Short(s) => s.encoded_size(),
            Variant::Int(i) => i.encoded_size(),
            Variant::Long(l) => l.encoded_size(),
            Variant::Float(f) => f.encoded_size(),
            Variant::Double(d) => d.encoded_size(),
            Variant::Char(c) => c.encoded_size(),
            Variant::Timestamp(ref t) => t.encoded_size(),
            Variant::Uuid(ref u) => u.encoded_size(),
            Variant::Binary(ref b) => b.encoded_size(),
            Variant::String(ref s) => s.encoded_size(),
            Variant::Symbol(ref s) => s.encoded_size(),
            Variant::StaticSymbol(ref s) => s.encoded_size(),
            Variant::List(ref l) => l.encoded_size(),
            Variant::Map(ref m) => m.map.encoded_size(),
            Variant::Described(ref dv) => dv.0.encoded_size() + dv.1.encoded_size(),
        }
    }

    /// Encodes `Variant` into provided `BytesMut`
    fn encode(&self, buf: &mut BytesMut) {
        match *self {
            Variant::Null => encode_null(buf),
            Variant::Boolean(b) => b.encode(buf),
            Variant::Ubyte(b) => b.encode(buf),
            Variant::Ushort(s) => s.encode(buf),
            Variant::Uint(i) => i.encode(buf),
            Variant::Ulong(l) => l.encode(buf),
            Variant::Byte(b) => b.encode(buf),
            Variant::Short(s) => s.encode(buf),
            Variant::Int(i) => i.encode(buf),
            Variant::Long(l) => l.encode(buf),
            Variant::Float(f) => f.encode(buf),
            Variant::Double(d) => d.encode(buf),
            Variant::Char(c) => c.encode(buf),
            Variant::Timestamp(ref t) => t.encode(buf),
            Variant::Uuid(ref u) => u.encode(buf),
            Variant::Binary(ref b) => b.encode(buf),
            Variant::String(ref s) => s.encode(buf),
            Variant::Symbol(ref s) => s.encode(buf),
            Variant::StaticSymbol(ref s) => s.encode(buf),
            Variant::List(ref l) => l.encode(buf),
            Variant::Map(ref m) => m.map.encode(buf),
            Variant::Described(ref dv) => {
                dv.0.encode(buf);
                dv.1.encode(buf);
            }
        }
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encoded_size(&self) -> usize {
        self.as_ref().map_or(1, |v| v.encoded_size())
    }

    fn encode(&self, buf: &mut BytesMut) {
        match *self {
            Some(ref e) => e.encode(buf),
            None => encode_null(buf),
        }
    }
}

impl Encode for Descriptor {
    fn encoded_size(&self) -> usize {
        match *self {
            Descriptor::Ulong(v) => 1 + v.encoded_size(),
            Descriptor::Symbol(ref v) => 1 + v.encoded_size(),
        }
    }

    fn encode(&self, buf: &mut BytesMut) {
        buf.put_u8(codec::FORMATCODE_DESCRIBED);
        match *self {
            Descriptor::Ulong(v) => v.encode(buf),
            Descriptor::Symbol(ref v) => v.encode(buf),
        }
    }
}

const WORD_LEN: usize = 4;

impl Encode for AmqpFrame {
    fn encoded_size(&self) -> usize {
        framing::HEADER_LEN + self.performative().encoded_size()
    }

    fn encode(&self, buf: &mut BytesMut) {
        let doff: u8 = (framing::HEADER_LEN / WORD_LEN) as u8;
        buf.put_u32(self.encoded_size() as u32);
        buf.put_u8(doff);
        buf.put_u8(framing::FRAME_TYPE_AMQP);
        buf.put_u16(self.channel_id());
        self.performative().encode(buf);
    }
}

impl Encode for SaslFrame {
    fn encoded_size(&self) -> usize {
        framing::HEADER_LEN + self.body.encoded_size()
    }

    fn encode(&self, buf: &mut BytesMut) {
        let doff: u8 = (framing::HEADER_LEN / WORD_LEN) as u8;
        buf.put_u32(self.encoded_size() as u32);
        buf.put_u8(doff);
        buf.put_u8(framing::FRAME_TYPE_SASL);
        buf.put_u16(0);
        self.body.encode(buf);
    }
}
