extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate futures;
#[macro_use]
extern crate failure;

#[cfg(test)]
extern crate http;

extern crate prost;
#[cfg(test)]
#[macro_use] extern crate prost_derive;

use std::fmt;
use std::ops::{Deref, DerefMut};

use bytes::{BytesMut, IntoBuf};
use prost::Message;
use prost::DecodeError as ProtoBufDecodeError;
use prost::EncodeError as ProtoBufEncodeError;

use futures::{Poll, Future, Stream};
use actix_web::http::header::{CONTENT_TYPE, CONTENT_LENGTH};
use actix_web::{Responder, HttpMessage, HttpRequest, HttpResponse, FromRequest};
use actix_web::dev::HttpResponseBuilder;
use actix_web::error::{Error, PayloadError, ResponseError};


#[derive(Fail, Debug)]
pub enum ProtoBufPayloadError {
    /// Payload size is bigger than 256k
    #[fail(display="Payload size is bigger than 256k")]
    Overflow,
    /// Content type error
    #[fail(display="Content type error")]
    ContentType,
    /// Serialize error
    #[fail(display="ProtoBuf serialize error: {}", _0)]
    Serialize(#[cause] ProtoBufEncodeError),
    /// Deserialize error
    #[fail(display="ProtoBuf deserialize error: {}", _0)]
    Deserialize(#[cause] ProtoBufDecodeError),
    /// Payload error
    #[fail(display="Error that occur during reading payload: {}", _0)]
    Payload(#[cause] PayloadError),
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

impl<T, S> FromRequest<S> for ProtoBuf<T>
    where T: Message + Default + 'static, S: 'static
{
    type Config = ProtoBufConfig;
    type Result = Box<Future<Item=Self, Error=Error>>;

    #[inline]
    fn from_request(req: &HttpRequest<S>, cfg: &Self::Config) -> Self::Result {
        Box::new(
            ProtoBufMessage::new(req)
                .limit(cfg.limit)
                .from_err()
                .map(ProtoBuf))
    }
}

impl<T: Message + Default> Responder for ProtoBuf<T> {
    type Item = HttpResponse;
    type Error = Error;

    fn respond_to<S>(self, _: &HttpRequest<S>) -> Result<HttpResponse, Error> {
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

pub struct ProtoBufMessage<T: HttpMessage, U: Message + Default>{
    limit: usize,
    length: Option<usize>,
    stream: Option<T::Stream>,
    err: Option<ProtoBufPayloadError>,
    fut: Option<Box<Future<Item=U, Error=ProtoBufPayloadError>>>,
}

impl<T: HttpMessage, U: Message + Default> ProtoBufMessage<T, U> {

    /// Create `ProtoBufMessage` for request.
    pub fn new(req: &T) -> Self {
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
            stream: Some(req.payload()),
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

impl<T, U: Message + Default + 'static> Future for ProtoBufMessage<T, U>
where T: HttpMessage + 'static
{
    type Item = U;
    type Error = ProtoBufPayloadError;

    fn poll(&mut self) -> Poll<U, ProtoBufPayloadError> {
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
            }).and_then(|body| Ok(<U>::decode(&mut body.into_buf())?));
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



pub trait ProtoBufHttpMessage {
    fn protobuf<T: Message + Default>(&self) -> ProtoBufMessage<Self, T>
        where Self: HttpMessage + 'static;
}

impl<S> ProtoBufHttpMessage for HttpRequest<S> {

    #[inline]
    fn protobuf<T: Message + Default>(&self) -> ProtoBufMessage<Self, T>
        where Self: HttpMessage + 'static
    {
        ProtoBufMessage::new(self)
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use http::header;
    use actix_web::test::TestRequest;

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
        let resp = protobuf.respond_to(&TestRequest::default().finish()).unwrap();
        assert_eq!(resp.headers().get(header::CONTENT_TYPE).unwrap(), "application/protobuf");
    }

    #[test]
    fn test_protobuf_message() {
        let req = TestRequest::default().finish();
        let mut protobuf = req.protobuf::<MyObject>();
        assert_eq!(protobuf.poll().err().unwrap(), ProtoBufPayloadError::ContentType);

        let req = TestRequest::default()
                        .header(
                            header::CONTENT_TYPE,
                            header::HeaderValue::from_static("application/text"),
                        ).finish();
        let mut protobuf = req.protobuf::<MyObject>();
        assert_eq!(protobuf.poll().err().unwrap(), ProtoBufPayloadError::ContentType);

        let req = TestRequest::default()
                        .header(
                            header::CONTENT_TYPE,
                            header::HeaderValue::from_static("application/protobuf"),
                        ).header(
                            header::CONTENT_LENGTH,
                            header::HeaderValue::from_static("10000"),
                        ).finish();
        let mut protobuf = req.protobuf::<MyObject>().limit(100);
        assert_eq!(protobuf.poll().err().unwrap(), ProtoBufPayloadError::Overflow);
    }
}
