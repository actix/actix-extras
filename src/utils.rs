use std::borrow::Cow;

use bytes::{BufMut, BytesMut};

// `bytes::Buf` is not implemented for `Cow<'static, str>`, implementing it by
// ourselves.
#[inline]
#[allow(clippy::ptr_arg)] // Totally okay to accept the reference to Cow here
pub fn put_cow(buf: &mut BytesMut, value: &Cow<'static, str>) {
    match value {
        Cow::Borrowed(str) => buf.put(str),
        Cow::Owned(ref string) => buf.put(string),
    }
}
