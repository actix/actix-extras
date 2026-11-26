use std::future::{ready, Ready};

use actix_web::{dev::Payload, Error, FromRequest, HttpRequest, HttpResponse};

use crate::{handle_payload, AggregatedMessageStream, Session};

/// Extracts an upgraded WebSocket connection.
///
/// Return `response` from the handler to complete the upgrade, and move `session` and `stream` into
/// a task to process messages. Extraction alone does not send the response or start a task.
///
/// Continuation frames are aggregated into complete messages. Use [`handle()`](crate::handle) to
/// receive individual frames instead or tweak frame/continuation size limits. The defaults should
/// work for most use cases.
///
/// This extractor takes ownership of the request payload, so it cannot be combined with another
/// body extractor.
///
/// For subprotocol negotiation, use [`handle_with_protocols()`] instead.
///
/// # Errors
///
/// Failed WebSocket handshakes return an error during extraction:
///
/// - `405 Method Not Allowed` if the request method is not `GET`.
/// - `426 Upgrade Required` if the WebSocket version is unsupported.
/// - `400 Bad Request` for other missing or invalid handshake headers.
///
/// # Examples
///
/// ```no_run
/// use actix_web::{HttpResponse, rt};
/// use actix_ws::{AggregatedMessage, WebSocket};
///
/// async fn ws(WebSocket { response, mut session, mut stream }: WebSocket) -> HttpResponse {
///     rt::spawn(async move {
///         while let Some(Ok(message)) = stream.recv().await {
///             match message {
///                 AggregatedMessage::Ping(bytes) => {
///                     if session.pong(&bytes).await.is_err() {
///                         return;
///                     }
///                 }
///                 AggregatedMessage::Close(reason) => {
///                     let _ = session.close(reason).await;
///                     return;
///                 }
///                 _ => {}
///             }
///         }
///
///         let _ = session.close(None).await;
///     });
///
///     response
/// }
/// ```
///
/// [`handle_with_protocols()`]: crate::handle_with_protocols
pub struct WebSocket {
    /// Handshake response that the handler must return to complete the upgrade.
    pub response: HttpResponse,

    /// Session used to send messages to the client.
    pub session: Session,

    /// Stream of messages from the client, with continuations aggregated.
    pub stream: AggregatedMessageStream,
}

impl FromRequest for WebSocket {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        ready(
            handle_payload(req, payload.take(), &[]).map(|(response, session, stream)| Self {
                response,
                session,
                stream: stream.aggregate_continuations(),
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{
        http::{header, StatusCode},
        test, web, App, HttpResponse,
    };

    use super::WebSocket;

    #[actix_web::test]
    async fn extractor_connects_payload_and_session() {
        use actix_web::{body::to_bytes, FromRequest};

        let (req, mut payload) = test::TestRequest::get()
            .insert_header((header::UPGRADE, "websocket"))
            .insert_header((header::CONNECTION, "upgrade"))
            .insert_header((header::SEC_WEBSOCKET_VERSION, "13"))
            .insert_header((header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ=="))
            // Masked text and continuation frames containing "h" and "i".
            .set_payload(web::Bytes::from_static(&[
                0x01,
                0x81,
                1,
                2,
                3,
                4,
                b'h' ^ 1,
                0x80,
                0x81,
                1,
                2,
                3,
                4,
                b'i' ^ 1,
            ]))
            .to_http_parts();

        let WebSocket {
            response,
            mut session,
            mut stream,
        } = WebSocket::from_request(&req, &mut payload).await.unwrap();

        assert!(matches!(payload, actix_web::dev::Payload::None));
        assert!(
            matches!(stream.recv().await, Some(Ok(crate::AggregatedMessage::Text(text))) if text == "hi")
        );

        session.text("hello").await.unwrap();
        session.close(None).await.unwrap();

        let body = to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body.as_ref(), b"\x81\x05hello\x88\x00");
    }

    async fn upgrade(ws: WebSocket) -> HttpResponse {
        ws.response
    }

    #[actix_web::test]
    async fn extractor_accepts_websocket_handshake() {
        let app = test::init_service(App::new().route("/", web::get().to(upgrade))).await;
        let req = test::TestRequest::get()
            .insert_header((header::UPGRADE, "websocket"))
            .insert_header((header::CONNECTION, "upgrade"))
            .insert_header((header::SEC_WEBSOCKET_VERSION, "13"))
            .insert_header((header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ=="))
            .to_request();

        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        assert_eq!(
            response
                .headers()
                .get(header::SEC_WEBSOCKET_ACCEPT)
                .unwrap(),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[actix_web::test]
    async fn extractor_rejects_invalid_handshake_before_handler() {
        let app = test::init_service(App::new().route(
            "/",
            web::get().to(|_: WebSocket| async {
                panic!("invalid handshake reached handler");
                #[allow(unreachable_code)]
                HttpResponse::Ok().finish()
            }),
        ))
        .await;

        let response = test::call_service(&app, test::TestRequest::get().to_request()).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
