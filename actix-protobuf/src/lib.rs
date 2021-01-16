#![deny(rust_2018_idioms)]

use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
};

use actix_web::dev::{HttpResponseBuilder, Payload};
use actix_web::error::{Error, PayloadError, ResponseError};
use actix_web::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::web::BytesMut;
use actix_web::{FromRequest, HttpMessage, HttpRequest, HttpResponse, Responder};
use bytes::BytesMut;
use derive_more::Display;
use futures_core::{ready, Stream};
use prost::DecodeError as ProtoBufDecodeError;
use prost::EncodeError as ProtoBufEncodeError;
use prost::Message;

#[derive(Debug, Display)]
pub enum ProtoBufPayloadError {
    /// Payload size is bigger than 256k
    #[display(fmt = "Payload size is bigger than 256k")]
    Overflow,
    /// Content type error
    #[display(fmt = "Content type error")]
    ContentType,
    /// Serialize error
    #[display(fmt = "ProtoBuf serialize error: {}", _0)]
    Serialize(ProtoBufEncodeError),
    /// Deserialize error
    #[display(fmt = "ProtoBuf deserialize error: {}", _0)]
    Deserialize(ProtoBufDecodeError),
    /// Payload error
    #[display(fmt = "Error that occur during reading payload: {}", _0)]
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

impl<T: Message> fmt::Debug for ProtoBuf<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProtoBuf: {:?}", self.0)
    }
}

impl<T: Message> fmt::Display for ProtoBuf<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
        ProtoBufConfig { limit: 262_144 }
    }
}

impl<T> FromRequest for ProtoBuf<T>
where
    T: Message + Default + 'static,
{
    type Config = ProtoBufConfig;
    type Error = Error;
    type Future = ProtoBufFuture<T>;

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let limit = req
            .app_data::<ProtoBufConfig>()
            .map(|c| c.limit)
            .unwrap_or(262_144);

        ProtoBufFuture {
            fut: ProtoBufMessage::new(req, payload).limit(limit),
        }
    }
}

pin_project_lite::pin_project! {
    pub struct ProtoBufFuture<T>
    where
        T: Message,
        T: Default
    {
        #[pin]
        fut: ProtoBufMessage<T>
    }
}

impl<T: Message + Default + 'static> Future for ProtoBufFuture<T> {
    type Output = Result<ProtoBuf<T>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().fut.poll(cx))?;
        Poll::Ready(Ok(ProtoBuf(res)))
    }
}

impl<T: Message + Default> Responder for ProtoBuf<T> {
    fn respond_to(self, _: &HttpRequest) -> HttpResponse {
        let mut buf = Vec::new();
        match self.0.encode(&mut buf) {
            Ok(()) => HttpResponse::Ok()
                .content_type("application/protobuf")
                .body(buf),
            Err(e) => {
                HttpResponse::from_error(ProtoBufPayloadError::Serialize(e).into())
            }
        }
    }
}

pub struct ProtoBufMessage<T: Message + Default> {
    limit: usize,
    length: Option<usize>,
    buf: BytesMut,
    res: Result<Payload, Option<ProtoBufPayloadError>>,
    _msg: PhantomData<T>,
}

impl<T: Message + Default> ProtoBufMessage<T> {
    /// Create `ProtoBufMessage` for request.
    pub fn new(req: &HttpRequest, payload: &mut Payload) -> Self {
        if req.content_type() != "application/protobuf" {
            return ProtoBufMessage {
                limit: 262_144,
                length: None,
                buf: BytesMut::new(),
                res: Err(Some(ProtoBufPayloadError::ContentType)),
                _msg: PhantomData,
            };
        }

        // Notice limit is not check against length here. ProtoBufMessage::limit is
        // always called after new and length/limit check happens there.
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
            buf: BytesMut::with_capacity(8192),
            res: Ok(payload.take()),
            _msg: PhantomData,
        }
    }

    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(mut self, limit: usize) -> Self {
        if let Some(len) = self.length {
            if len > limit {
                self.res = Err(Some(ProtoBufPayloadError::Overflow));
            }
        }
        self.limit = limit;
        self
    }
}

impl<T: Message + Default + 'static> Unpin for ProtoBufMessage<T> {}

impl<T: Message + Default + 'static> Future for ProtoBufMessage<T> {
    type Output = Result<T, ProtoBufPayloadError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut stream = match this.res.as_mut() {
            Ok(stream) => Pin::new(stream),
            Err(e) => {
                return Poll::Ready(Err(e
                    .take()
                    .expect("ProtoBufMessage polled after finished")))
            }
        };

        loop {
            match ready!(stream.as_mut().poll_next(cx)) {
                Some(item) => {
                    let chunk = item?;
                    if (this.buf.len() + chunk.len()) > this.limit {
                        return Poll::Ready(Err(ProtoBufPayloadError::Overflow));
                    } else {
                        this.buf.extend_from_slice(&chunk);
                    }
                }
                None => return Poll::Ready(Ok(<T>::decode(&mut this.buf)?)),
            }
        }
    }
}

pub trait ProtoBufResponseBuilder {
    fn protobuf<T: Message>(&mut self, value: T) -> Result<HttpResponse, Error>;
}

impl ProtoBufResponseBuilder for HttpResponseBuilder {
    fn protobuf<T: Message>(&mut self, value: T) -> Result<HttpResponse, Error> {
        self.header(CONTENT_TYPE, "application/protobuf");

        let mut body = Vec::new();
        value
            .encode(&mut body)
            .map_err(ProtoBufPayloadError::Serialize)?;
        Ok(self.body(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header;
    use actix_web::test::TestRequest;

    impl PartialEq for ProtoBufPayloadError {
        fn eq(&self, other: &ProtoBufPayloadError) -> bool {
            match *self {
                ProtoBufPayloadError::Overflow => {
                    matches!(*other, ProtoBufPayloadError::Overflow)
                }
                ProtoBufPayloadError::ContentType => {
                    matches!(*other, ProtoBufPayloadError::ContentType)
                }
                _ => false,
            }
        }
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct MyObject {
        #[prost(int32, tag = "1")]
        pub number: i32,
        #[prost(string, tag = "2")]
        pub name: String,
    }

    #[actix_rt::test]
    async fn test_protobuf() {
        let protobuf = ProtoBuf(MyObject {
            number: 9,
            name: "test".to_owned(),
        });
        let req = TestRequest::default().to_http_request();
        let resp = protobuf.respond_to(&req).await.unwrap();
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/protobuf"
        );
    }

    #[actix_rt::test]
    async fn test_protobuf_message() {
        let (req, mut pl) = TestRequest::default().to_http_parts();
        let protobuf = ProtoBufMessage::<MyObject>::new(&req, &mut pl).await;
        assert_eq!(protobuf.err().unwrap(), ProtoBufPayloadError::ContentType);

        let (req, mut pl) =
            TestRequest::with_header(header::CONTENT_TYPE, "application/text")
                .to_http_parts();
        let protobuf = ProtoBufMessage::<MyObject>::new(&req, &mut pl).await;
        assert_eq!(protobuf.err().unwrap(), ProtoBufPayloadError::ContentType);

        let (req, mut pl) =
            TestRequest::with_header(header::CONTENT_TYPE, "application/protobuf")
                .header(header::CONTENT_LENGTH, "10000")
                .to_http_parts();
        let protobuf = ProtoBufMessage::<MyObject>::new(&req, &mut pl)
            .limit(100)
            .await;
        assert_eq!(protobuf.err().unwrap(), ProtoBufPayloadError::Overflow);
    }
}
