use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use actix_http::body::EitherBody;
use actix_http::{Payload, StatusCode};
use actix_web::error::PayloadError;
use actix_web::http::header::CONTENT_LENGTH;
use actix_web::web::BytesMut;
use actix_web::{web, FromRequest, HttpRequest, HttpResponse, Responder, ResponseError};
use derive_more::Display;
use futures_core::{ready, stream::Stream};
use prost::Message;

const DEFAULT_LIMIT: usize = 2_097_152;
const CONTENT_TYPE: &str = "application/protobuf";

#[derive(Debug, Display, derive_more::Error)]
#[non_exhaustive]
pub enum ProtoPayloadError {
    /// Payload size is bigger than allowed & content length header set. (default: 2MB)
    #[display(
        fmt = "ProtoBuf payload ({} bytes) is larger than allowed (limit: {} bytes).",
        length,
        limit
    )]
    OverflowKnownLength { length: usize, limit: usize },

    /// Payload size is bigger than allowed but no content length header set. (default: 2MB)
    #[display(fmt = "ProtoBuf payload has exceeded limit ({} bytes).", limit)]
    Overflow { limit: usize },

    /// Content type error
    #[display(fmt = "Content type error")]
    ContentType,

    /// Deserialize error
    #[display(fmt = "ProtoBuf deserialize error: {}", _0)]
    Deserialize(prost::DecodeError),

    /// Serialize error
    #[display(fmt = "ProtoBuf serialize error: {}", _0)]
    Serialize(prost::EncodeError),

    /// Payload error
    #[display(fmt = "Error that occur during reading payload: {}", _0)]
    Payload(PayloadError),
}

impl From<PayloadError> for ProtoPayloadError {
    fn from(err: PayloadError) -> Self {
        Self::Payload(err)
    }
}

/// Return `BadRequest` for `ProtoPayloadError`
impl ResponseError for ProtoPayloadError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::OverflowKnownLength {
                length: _,
                limit: _,
            } => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Overflow { limit: _ } => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Serialize(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Payload(err) => err.status_code(),
            _ => StatusCode::BAD_REQUEST,
        }
    }
}

pub type ProtoContentTypeHandler = Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>;

#[derive(Clone)]
pub struct ProtoConfig {
    limit: usize,
    err_handler: ProtoErrorHandler,
    content_type: ProtoContentTypeHandler,
    content_type_required: bool,
}

impl ProtoConfig {
    /// Set maximum accepted payload size. By default this limit is 2MB.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set custom error handler.
    pub fn error_handler<F>(mut self, f: F) -> Self
    where
        F: Fn(ProtoPayloadError, &HttpRequest) -> actix_web::Error + Send + Sync + 'static,
    {
        self.err_handler = Some(Arc::new(f));
        self
    }

    /// Set predicate for allowed content types.
    pub fn content_type<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.content_type = Some(Arc::new(predicate));
        self
    }

    /// Sets whether or not the request must have a `Content-Type` header to be parsed.
    pub fn content_type_required(mut self, content_type_required: bool) -> Self {
        self.content_type_required = content_type_required;
        self
    }

    /// Extract payload config from app data. Check both `T` and `Data<T>`, in that order, and fall
    /// back to the default payload config.
    fn from_req(req: &HttpRequest) -> &Self {
        req.app_data::<Self>()
            .or_else(|| req.app_data::<web::Data<Self>>().map(|d| d.as_ref()))
            .unwrap_or(&DEFAULT_CONFIG)
    }
}

/// Allow shared refs used as default.
const DEFAULT_CONFIG: ProtoConfig = ProtoConfig {
    limit: DEFAULT_LIMIT,
    err_handler: None,
    content_type: None,
    content_type_required: true,
};

impl Default for ProtoConfig {
    fn default() -> Self {
        DEFAULT_CONFIG.clone()
    }
}

#[derive(Clone, Debug)]
pub struct ProtoBuf<T: Message> {
    pub message: T,
}

impl<T: Message> Responder for ProtoBuf<T> {
    type Body = EitherBody<Vec<u8>>;

    fn respond_to(self, _: &HttpRequest) -> HttpResponse<Self::Body> {
        let body = self.message.encode_to_vec();
        match HttpResponse::Ok()
            .content_type(CONTENT_TYPE)
            .message_body(body)
        {
            Ok(res) => res.map_into_left_body(),
            Err(err) => HttpResponse::from_error(err).map_into_right_body(),
        }
    }
}

impl<T: Message> Deref for ProtoBuf<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl<T: Message> DerefMut for ProtoBuf<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.message
    }
}

impl<T: Message> ProtoBuf<T> {
    pub fn new(message: T) -> Self {
        Self { message }
    }

    pub fn into_inner(self) -> T {
        self.message
    }
}

impl<T: Message + Default> FromRequest for ProtoBuf<T> {
    type Error = actix_web::Error;
    type Future = ProtoExtractFut<T>;

    #[inline]
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let config = ProtoConfig::from_req(req);

        let limit = config.limit;
        let ctype_required = config.content_type_required;
        let ctype_fn = config.content_type.as_deref();
        let err_handler = config.err_handler.clone();

        ProtoExtractFut {
            req: Some(req.clone()),
            fut: ProtoBody::new(req, payload, ctype_fn, ctype_required).limit(limit),
            err_handler,
        }
    }
}

pub enum ProtoBody<T> {
    Error(Option<ProtoPayloadError>),
    Body {
        limit: usize,
        length: Option<usize>,
        #[cfg(feature = "__compress")]
        payload: Decompress<Payload>,
        #[cfg(not(feature = "__compress"))]
        payload: Payload,
        buf: BytesMut,
        _res: Pin<Box<PhantomData<T>>>,
    },
}

impl<T: Message> ProtoBody<T> {
    #[allow(clippy::borrow_interior_mutable_const)]
    pub fn new(
        req: &HttpRequest,
        payload: &mut Payload,
        ctype_fn: Option<&(dyn Fn(&str) -> bool + Send + Sync)>,
        ctype_required: bool,
    ) -> Self {
        let can_parse_proto = !ctype_required
            || req.headers().get(actix_http::header::CONTENT_TYPE).map_or(
                false,
                |content_type_header| {
                    content_type_header
                        .to_str()
                        .map_or(false, |content_type_str| {
                            content_type_str == CONTENT_TYPE
                                || ctype_fn.map_or(false, |predicate| predicate(content_type_str))
                        })
                },
            );

        if !can_parse_proto {
            return ProtoBody::Error(Some(ProtoPayloadError::ContentType));
        }

        let length = req
            .headers()
            .get(&CONTENT_LENGTH)
            .and_then(|l| l.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok());

        let payload = {
            cfg_if::cfg_if! {
                if #[cfg(feature = "__compress")] {
                    Decompress::from_headers(payload.take(), req.headers())
                } else {
                    payload.take()
                }
            }
        };

        ProtoBody::Body {
            limit: DEFAULT_LIMIT,
            length,
            payload,
            buf: BytesMut::with_capacity(8192),
            _res: Box::pin(PhantomData),
        }
    }

    /// Set maximum accepted payload size. The default limit is 2MB.
    pub fn limit(self, limit: usize) -> Self {
        match self {
            ProtoBody::Body {
                length,
                payload,
                buf,
                ..
            } => {
                if let Some(len) = length {
                    if len > limit {
                        return ProtoBody::Error(Some(ProtoPayloadError::OverflowKnownLength {
                            length: len,
                            limit,
                        }));
                    }
                }

                ProtoBody::Body {
                    limit,
                    length,
                    payload,
                    buf,
                    _res: Box::pin(PhantomData),
                }
            }
            ProtoBody::Error(e) => ProtoBody::Error(e),
        }
    }
}

impl<T: Message + Default> Future for ProtoBody<T> {
    type Output = Result<T, ProtoPayloadError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        match this {
            ProtoBody::Body {
                limit,
                buf,
                payload,
                ..
            } => loop {
                let res = ready!(Pin::new(&mut *payload).poll_next(cx));
                match res {
                    Some(chunk) => {
                        let chunk = chunk.unwrap();
                        let buf_len = buf.len() + chunk.len();
                        if buf_len > *limit {
                            return Poll::Ready(Err(ProtoPayloadError::Overflow { limit: *limit }));
                        } else {
                            buf.extend_from_slice(&chunk);
                        }
                    }
                    None => {
                        let proto = T::decode(buf).map_err(ProtoPayloadError::Deserialize)?;
                        return Poll::Ready(Ok(proto));
                    }
                }
            },
            ProtoBody::Error(e) => Poll::Ready(Err(e.take().unwrap())),
        }
    }
}

pub type ProtoErrorHandler =
    Option<Arc<dyn Fn(ProtoPayloadError, &HttpRequest) -> actix_web::Error + Send + Sync>>;

pub struct ProtoExtractFut<T> {
    req: Option<HttpRequest>,
    fut: ProtoBody<T>,
    err_handler: ProtoErrorHandler,
}

impl<T: Message + Default> Future for ProtoExtractFut<T> {
    type Output = Result<ProtoBuf<T>, actix_web::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        let res = ready!(Pin::new(&mut this.fut).poll(cx));

        let res = match res {
            Err(err) => {
                let req = this.req.take().unwrap();
                if let Some(err_handler) = this.err_handler.as_ref() {
                    Err((*err_handler)(err, &req))
                } else {
                    Err(err.into())
                }
            }
            Ok(data) => Ok(ProtoBuf::new(data)),
        };

        Poll::Ready(res)
    }
}

#[cfg(test)]
mod tests {
    use actix_http::body;
    use actix_web::{error::InternalError, http::header, test::TestRequest};

    use super::*;

    macro_rules! assert_body_eq {
        ($res:ident, $expected:ident) => {
            assert_eq!(
                ::actix_http::body::to_bytes($res.into_body())
                    .await
                    .expect("error reading test response body"),
                $expected.clone().encode_to_vec()
            )
        };
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct MyObject {
        #[prost(message, optional, tag = "1")]
        pub name: ::core::option::Option<::prost::alloc::string::String>,
    }

    fn proto_eq(err: ProtoPayloadError, other: ProtoPayloadError) -> bool {
        match err {
            ProtoPayloadError::Overflow { .. } => {
                matches!(other, ProtoPayloadError::Overflow { .. })
            }
            ProtoPayloadError::OverflowKnownLength { .. } => {
                matches!(other, ProtoPayloadError::OverflowKnownLength { .. })
            }
            ProtoPayloadError::ContentType => matches!(other, ProtoPayloadError::ContentType),
            _ => false,
        }
    }

    #[actix_rt::test]
    async fn test_responder() {
        let req = TestRequest::default().to_http_request();

        let response_value = ProtoBuf::new(MyObject {
            name: Some("test".to_string()),
        });
        let res = response_value.clone().respond_to(&req);
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            header::HeaderValue::from_static(CONTENT_TYPE)
        );
        assert_body_eq!(res, response_value);
    }

    #[actix_rt::test]
    async fn test_custom_error_responder() {
        let proto_payload = ProtoBuf::new(MyObject {
            name: Some("This message here is long".to_string()),
        });
        let (req, mut payload) = TestRequest::default()
            .set_payload(proto_payload.encode_to_vec())
            .app_data(
                ProtoConfig::default()
                    .content_type_required(false)
                    .limit(10)
                    .error_handler(|err, _| {
                        let msg = ProtoBuf::new(MyObject {
                            name: Some("invalid request".to_string()),
                        });
                        let resp = HttpResponse::BadRequest().body(msg.encode_to_vec());
                        InternalError::from_response(err, resp).into()
                    }),
            )
            .to_http_parts();

        let from_req = ProtoBuf::<MyObject>::from_request(&req, &mut payload).await;
        let resp = HttpResponse::from_error(from_req.unwrap_err());
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = body::to_bytes(resp.into_body()).await.unwrap();
        let msg: MyObject = MyObject::decode::<&[u8]>(&body.to_vec()[..]).unwrap();
        assert_eq!(msg.name, Some("invalid request".to_string()));
    }

    #[actix_rt::test]
    async fn test_extract() {
        let expected_value = Some("exists".to_string());
        let proto_payload = ProtoBuf::new(MyObject {
            name: expected_value.clone(),
        });
        let (req, mut payload) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(CONTENT_TYPE),
            ))
            .set_payload(proto_payload.encode_to_vec())
            .to_http_parts();

        let from_request = ProtoBuf::<MyObject>::from_request(&req, &mut payload)
            .await
            .unwrap();
        assert_eq!(
            from_request.into_inner(),
            MyObject {
                name: expected_value
            }
        );
    }

    #[actix_rt::test]
    async fn test_extract_payload_larger_than_limit() {
        let expected_value = Some("eleven_".to_string());
        let proto_payload = ProtoBuf::new(MyObject {
            name: expected_value.clone(),
        });
        let (req, mut payload) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(CONTENT_TYPE),
            ))
            .app_data(ProtoConfig::default().limit(10))
            .set_payload(proto_payload.encode_to_vec())
            .to_http_parts();

        let from_request = ProtoBuf::<MyObject>::from_request(&req, &mut payload).await;
        assert_eq!(
            format!("{}", from_request.unwrap_err()),
            "ProtoBuf payload has exceeded limit (10 bytes).".to_string()
        );
    }

    #[actix_rt::test]
    async fn test_extract_payload_content_length_larger_than_limit() {
        let expected_value = Some("sixteen_len_".to_string());
        let proto_payload = ProtoBuf::new(MyObject {
            name: expected_value.clone(),
        });
        let (req, mut payload) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(CONTENT_TYPE),
            ))
            .insert_header((
                header::CONTENT_LENGTH,
                header::HeaderValue::from_static("16"),
            ))
            .app_data(ProtoConfig::default().limit(10))
            .set_payload(proto_payload.encode_to_vec())
            .to_http_parts();

        let from_request = ProtoBuf::<MyObject>::from_request(&req, &mut payload).await;
        assert_eq!(
            format!("{}", from_request.unwrap_err()),
            "ProtoBuf payload (16 bytes) is larger than allowed (limit: 10 bytes).".to_string()
        );
    }

    #[actix_rt::test]
    async fn test_extract_payload_content_length_larger_than_limit_struct() {
        let (req, mut pl) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(CONTENT_TYPE),
            ))
            .insert_header((
                header::CONTENT_LENGTH,
                header::HeaderValue::from_static("16"),
            ))
            .to_http_parts();

        let proto = ProtoBody::<MyObject>::new(&req, &mut pl, None, true)
            .limit(10)
            .await;

        assert!(proto_eq(
            proto.err().unwrap(),
            ProtoPayloadError::OverflowKnownLength {
                length: 16,
                limit: 10
            }
        ))
    }

    #[actix_rt::test]
    async fn test_extract_payload_content_length_larger_than_limit_bytes() {
        let (req, mut pl) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(CONTENT_TYPE),
            ))
            .set_payload(vec![0u8; 1000])
            .to_http_parts();

        let proto = ProtoBody::<MyObject>::new(&req, &mut pl, None, true)
            .limit(100)
            .await;

        println!("{:?}", proto);

        assert!(proto_eq(
            proto.err().unwrap(),
            ProtoPayloadError::Overflow { limit: 100 }
        ));
    }

    #[actix_rt::test]
    async fn test_proto_body_invalid_content_type_none() {
        let (req, mut pl) = TestRequest::default().to_http_parts();
        let proto = ProtoBody::<MyObject>::new(&req, &mut pl, None, true).await;
        assert!(proto_eq(
            proto.err().unwrap(),
            ProtoPayloadError::ContentType
        ));
    }

    #[actix_rt::test]
    async fn test_proto_body_invalid_content_type_text() {
        let (req, mut pl) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/text"),
            ))
            .to_http_parts();

        let proto = ProtoBody::<MyObject>::new(&req, &mut pl, None, true).await;
        assert!(proto_eq(proto.unwrap_err(), ProtoPayloadError::ContentType));
    }

    #[actix_rt::test]
    async fn test_proto_body_req_encode_decode() {
        let expected_value = Some("this works".to_string());
        let (req, mut pl) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(CONTENT_TYPE),
            ))
            .set_payload(
                MyObject {
                    name: expected_value.clone(),
                }
                .encode_to_vec(),
            )
            .to_http_parts();

        let proto = ProtoBody::<MyObject>::new(&req, &mut pl, None, true).await;
        assert_eq!(
            proto.ok().unwrap(),
            MyObject {
                name: expected_value.clone()
            }
        );
    }

    #[actix_rt::test]
    async fn test_with_custom_content_type() {
        let expected_value = Some("this works".to_string());
        let custom_ctype = "customcontenttype";
        let (req, mut pl) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static(custom_ctype.clone()),
            ))
            .set_payload(
                MyObject {
                    name: expected_value.clone(),
                }
                .encode_to_vec(),
            )
            .app_data(ProtoConfig::default().content_type(move |ctype| ctype == custom_ctype))
            .to_http_parts();

        let s = ProtoBuf::<MyObject>::from_request(&req, &mut pl).await;
        assert!(s.is_ok())
    }

    #[actix_rt::test]
    async fn test_with_bad_custom_content_type() {
        let expected_value = Some("this works".to_string());
        let (req, mut pl) = TestRequest::default()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("text/html"),
            ))
            .set_payload(
                MyObject {
                    name: expected_value.clone(),
                }
                .encode_to_vec(),
            )
            .app_data(
                ProtoConfig::default().content_type(move |ctype| ctype == "customcontenttype"),
            )
            .to_http_parts();

        let proto = ProtoBuf::<MyObject>::from_request(&req, &mut pl).await;
        assert_eq!(
            format!("{}", proto.err().unwrap()),
            format!("{}", ProtoPayloadError::ContentType)
        );
    }
}
