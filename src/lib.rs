extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate futures;
extern crate derive_more;

#[cfg(test)]
extern crate http;

extern crate prost;
#[cfg(test)]
#[macro_use] extern crate prost_derive;

use std::fmt;
use derive_more::Display;
use std::ops::{Deref, DerefMut};

use bytes::{BytesMut, IntoBuf};
use prost::Message;
use prost::DecodeError as ProtoBufDecodeError;
use prost::EncodeError as ProtoBufEncodeError;

use futures::{Poll, Future, Stream};
use actix_web::http::header::{CONTENT_TYPE, CONTENT_LENGTH};
use actix_web::{Responder, HttpMessage, HttpRequest, HttpResponse, FromRequest};
use actix_web::dev::{HttpResponseBuilder, Payload};
use actix_web::error::{Error, PayloadError, ResponseError};

#[derive(Debug, Display)]
pub enum ProtoBufPayloadError {
    /// Payload size is bigger than 256k
    #[display(fmt="Payload size is bigger than 256k")]
    Overflow,
    /// Content type error
    #[display(fmt="Content type error")]
    ContentType,
    /// Serialize error
    #[display(fmt="ProtoBuf serialize error: {}", _0)]
    Serialize(ProtoBufEncodeError),
    /// Deserialize error
    #[display(fmt="ProtoBuf deserialize error: {}", _0)]
    Deserialize(ProtoBufDecodeError),
    /// Payload error
    #[display(fmt="Error that occur during reading payload: {}", _0)]
    Payload(PayloadError),
}

impl ResponseError for ProtoBufPayloadError {

    fn error_response(&self) -> HttpResponse {
        match *self {
            ProtoBufPayloadError::Overflow => HttpResponse::PayloadTooLarge().into(),
            _ => HttpResponse::BadRequest().into(),
        }
    }
}

impl From<PayloadError> for ProtoBufPayloadError {
    fn from(err: PayloadError) -> ProtoBufPayloadError {
        ProtoBufPayloadError::Payload(err)
    }
}

impl From<ProtoBufDecodeError> for ProtoBufPayloadError {
    fn from(err: ProtoBufDecodeError) -> ProtoBufPayloadError {
        ProtoBufPayloadError::Deserialize(err)
    }
}

pub struct ProtoBuf<T: Message>(pub T);

impl<T: Message> Deref for ProtoBuf<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Message> DerefMut for ProtoBuf<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Message> fmt::Debug for ProtoBuf<T> where T: fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ProtoBuf: {:?}", self.0)
    }
}

impl<T: Message> fmt::Display for ProtoBuf<T> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

pub struct ProtoBufConfig {
    limit: usize,
}

impl ProtoBufConfig {

    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(&mut self, limit: usize) -> &mut Self {
        self.limit = limit;
        self
    }
}

impl Default for ProtoBufConfig {
    fn default() -> Self {
        ProtoBufConfig{limit: 262_144}
    }
}

impl<T> FromRequest for ProtoBuf<T>
    where T: Message + Default + 'static
{
    type Config = ProtoBufConfig;
    type Error = Error;
    type Future = Box<Future<Item=Self, Error=Error>>;

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let limit = req.app_data::<ProtoBufConfig>().map(|c| c.limit).unwrap_or(262_144);
        Box::new(
            ProtoBufMessage::new(req, payload)
                .limit(limit)
                .map_err(move |e| {
                    e.into()
                })
                .map(ProtoBuf))
    }
}

impl<T: Message + Default> Responder for ProtoBuf<T> {
    type Error = Error;
    type Future = Result<HttpResponse, Error>;

    fn respond_to(self, _: &HttpRequest) -> Self::Future {
        let mut buf = Vec::new();
        self.0.encode(&mut buf)
            .map_err(|e| Error::from(ProtoBufPayloadError::Serialize(e)))
            .and_then(|()| {
                Ok(HttpResponse::Ok()
                   .content_type("application/protobuf")
                   .body(buf))
            })
    }
}

pub struct ProtoBufMessage<T: Message + Default>{
    limit: usize,
    length: Option<usize>,
    stream: Option<Payload>,
    err: Option<ProtoBufPayloadError>,
    fut: Option<Box<Future<Item=T, Error=ProtoBufPayloadError>>>,
}

impl<T: Message + Default> ProtoBufMessage<T> {

    /// Create `ProtoBufMessage` for request.
    pub fn new(req: &HttpRequest, payload: &mut Payload) -> Self {
        if req.content_type() != "application/protobuf" {
            return ProtoBufMessage {
                limit: 262_144,
                length: None,
                stream: None,
                fut: None,
                err: Some(ProtoBufPayloadError::ContentType),
            };
        }

        let mut len = None;
        if let Some(l) = req.headers().get(CONTENT_LENGTH) {
            if let Ok(s) = l.to_str() {
                if let Ok(l) = s.parse::<usize>() {
                    len = Some(l)
                }
            }
        }

        ProtoBufMessage {
            limit: 262_144,
            length: len,
            stream: Some(payload.take()),
            fut: None,
            err: None,
        }
    }

    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

impl<T: Message + Default + 'static> Future for ProtoBufMessage<T>
{
    type Item = T;
    type Error = ProtoBufPayloadError;

    fn poll(&mut self) -> Poll<T, ProtoBufPayloadError> {
        if let Some(ref mut fut) = self.fut {
            return fut.poll();
        }

        if let Some(err) = self.err.take() {
            return Err(err);
        }

        let limit = self.limit;
        if let Some(len) = self.length.take() {
            if len > limit {
                return Err(ProtoBufPayloadError::Overflow);
            }
        }

        let fut = self
            .stream
            .take()
            .expect("ProtoBufMessage could not be used second time")
            .from_err()
            .fold(BytesMut::with_capacity(8192), move |mut body, chunk| {
                if (body.len() + chunk.len()) > limit {
                    Err(ProtoBufPayloadError::Overflow)
                } else {
                    body.extend_from_slice(&chunk);
                    Ok(body)
                }
            }).and_then(|body| Ok(<T>::decode(&mut body.into_buf())?));
        self.fut = Some(Box::new(fut));
        self.poll()
    }
}


pub trait ProtoBufResponseBuilder {

    fn protobuf<T: Message>(&mut self, value: T) -> Result<HttpResponse, Error>;
}

impl ProtoBufResponseBuilder for HttpResponseBuilder {

    fn protobuf<T: Message>(&mut self, value: T) -> Result<HttpResponse, Error> {
        self.header(CONTENT_TYPE, "application/protobuf");

        let mut body = Vec::new();
        value.encode(&mut body).map_err(ProtoBufPayloadError::Serialize)?;
        Ok(self.body(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header;
    use actix_web::test::{block_on, TestRequest};

    impl PartialEq for ProtoBufPayloadError {
        fn eq(&self, other: &ProtoBufPayloadError) -> bool {
            match *self {
                ProtoBufPayloadError::Overflow => match *other {
                    ProtoBufPayloadError::Overflow => true,
                    _ => false,
                },
                ProtoBufPayloadError::ContentType => match *other {
                    ProtoBufPayloadError::ContentType => true,
                    _ => false,
                },
                _ => false,
            }
        }
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct MyObject {
        #[prost(int32, tag="1")]
        pub number: i32,
        #[prost(string, tag="2")]
        pub name: String,
    }

    #[test]
    fn test_protobuf() {
        let protobuf = ProtoBuf(MyObject{number: 9 , name: "test".to_owned()});
        let req = TestRequest::default().to_http_request();
        let resp = protobuf.respond_to(&req).unwrap();
        assert_eq!(resp.headers().get(header::CONTENT_TYPE).unwrap(), "application/protobuf");
    }

    #[test]
    fn test_protobuf_message() {
        let (req, mut pl) = TestRequest::default().to_http_parts();
        let protobuf = block_on(ProtoBufMessage::<MyObject>::new(&req, &mut pl));
        assert_eq!(protobuf.err().unwrap(), ProtoBufPayloadError::ContentType);

        let (req, mut pl) = TestRequest::default()
                        .header(
                            header::CONTENT_TYPE,
                            header::HeaderValue::from_static("application/text"),
                        ).to_http_parts();
        let protobuf = block_on(ProtoBufMessage::<MyObject>::new(&req, &mut pl));
        assert_eq!(protobuf.err().unwrap(), ProtoBufPayloadError::ContentType);

        let (req, mut pl) = TestRequest::default()
                        .header(
                            header::CONTENT_TYPE,
                            header::HeaderValue::from_static("application/protobuf"),
                        ).header(
                            header::CONTENT_LENGTH,
                            header::HeaderValue::from_static("10000"),
                        ).to_http_parts();
        let protobuf = block_on(ProtoBufMessage::<MyObject>::new(&req, &mut pl).limit(100));
        assert_eq!(protobuf.err().unwrap(), ProtoBufPayloadError::Overflow);
    }
}
