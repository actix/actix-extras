use derive_more::Display;
use std::fmt;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task;
use std::task::Poll;

use bytes::BytesMut;
use prost::DecodeError as ProtoBufDecodeError;
use prost::EncodeError as ProtoBufEncodeError;
use prost::Message;

use actix_web::dev::{HttpResponseBuilder, Payload};
use actix_web::error::{Error, PayloadError, ResponseError};
use actix_web::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::{FromRequest, HttpMessage, HttpRequest, HttpResponse, Responder};
use futures_util::future::{ready, FutureExt, LocalBoxFuture, Ready};
use futures_util::StreamExt;

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ProtoBuf: {:?}", self.0)
    }
}

impl<T: Message> fmt::Display for ProtoBuf<T>
where
    T: fmt::Display,
{
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
        ProtoBufConfig { limit: 262_144 }
    }
}

impl<T> FromRequest for ProtoBuf<T>
where
    T: Message + Default + 'static,
{
    type Config = ProtoBufConfig;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Error>>;

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let limit = req
            .app_data::<ProtoBufConfig>()
            .map(|c| c.limit)
            .unwrap_or(262_144);
        ProtoBufMessage::new(req, payload)
            .limit(limit)
            .map(move |res| match res {
                Err(e) => Err(e.into()),
                Ok(item) => Ok(ProtoBuf(item)),
            })
            .boxed_local()
    }
}

impl<T: Message + Default> Responder for ProtoBuf<T> {
    type Error = Error;
    type Future = Ready<Result<HttpResponse, Error>>;

    fn respond_to(self, _: &HttpRequest) -> Self::Future {
        let mut buf = Vec::new();
        ready(
            self.0
                .encode(&mut buf)
                .map_err(|e| Error::from(ProtoBufPayloadError::Serialize(e)))
                .map(|()| {
                    HttpResponse::Ok()
                        .content_type("application/protobuf")
                        .body(buf)
                }),
        )
    }
}

pub struct ProtoBufMessage<T: Message + Default> {
    limit: usize,
    length: Option<usize>,
    stream: Option<Payload>,
    err: Option<ProtoBufPayloadError>,
    fut: Option<LocalBoxFuture<'static, Result<T, ProtoBufPayloadError>>>,
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

impl<T: Message + Default + 'static> Future for ProtoBufMessage<T> {
    type Output = Result<T, ProtoBufPayloadError>;

    fn poll(
        mut self: Pin<&mut Self>,
        task: &mut task::Context<'_>,
    ) -> Poll<Self::Output> {
        if let Some(ref mut fut) = self.fut {
            return Pin::new(fut).poll(task);
        }

        if let Some(err) = self.err.take() {
            return Poll::Ready(Err(err));
        }

        let limit = self.limit;
        if let Some(len) = self.length.take() {
            if len > limit {
                return Poll::Ready(Err(ProtoBufPayloadError::Overflow));
            }
        }

        let mut stream = self
            .stream
            .take()
            .expect("ProtoBufMessage could not be used second time");

        self.fut = Some(
            async move {
                let mut body = BytesMut::with_capacity(8192);

                while let Some(item) = stream.next().await {
                    let chunk = item?;
                    if (body.len() + chunk.len()) > limit {
                        return Err(ProtoBufPayloadError::Overflow);
                    } else {
                        body.extend_from_slice(&chunk);
                    }
                }

                return Ok(<T>::decode(&mut body)?);
            }
            .boxed_local(),
        );
        self.poll(task)
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
