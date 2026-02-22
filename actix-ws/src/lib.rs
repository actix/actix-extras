//! WebSockets for Actix Web, without actors.
//!
//! For usage, see documentation on [`handle()`] and [`handle_with_protocols()`].

#![warn(missing_docs)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use actix_http::ws::{CloseCode, CloseReason, Item, Message, ProtocolError};
use actix_http::{
    body::{BodyStream, MessageBody},
    ws::handshake,
};
use actix_web::{http::header, web, HttpRequest, HttpResponse};
use tokio::sync::mpsc::channel;

mod aggregated;
pub mod codec;
mod session;
mod stream;

pub use self::{
    aggregated::{AggregatedMessage, AggregatedMessageStream},
    session::{Closed, Session},
    stream::{MessageStream, StreamingBody},
};

/// Begin handling websocket traffic
///
/// To negotiate sub-protocols via `Sec-WebSocket-Protocol`, use [`handle_with_protocols`].
///
/// ```no_run
/// use std::io;
/// use actix_web::{middleware::Logger, web, App, HttpRequest, HttpServer, Responder};
/// use actix_ws::Message;
///
/// async fn ws(req: HttpRequest, body: web::Payload) -> actix_web::Result<impl Responder> {
///     let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;
///
///     actix_web::rt::spawn(async move {
///         while let Some(Ok(msg)) = msg_stream.recv().await {
///             match msg {
///                 Message::Ping(bytes) => {
///                     if session.pong(&bytes).await.is_err() {
///                         return;
///                     }
///                 }
///
///                 Message::Text(msg) => println!("Got text: {msg}"),
///                 _ => break,
///             }
///         }
///
///         let _ = session.close(None).await;
///     });
///
///     Ok(response)
/// }
///
/// #[tokio::main(flavor = "current_thread")]
/// async fn main() -> io::Result<()> {
///     HttpServer::new(move || {
///         App::new()
///             .route("/ws", web::get().to(ws))
///             .wrap(Logger::default())
///     })
///     .bind(("127.0.0.1", 8080))?
///     .run()
///     .await
/// }
/// ```
pub fn handle(
    req: &HttpRequest,
    body: web::Payload,
) -> Result<(HttpResponse, Session, MessageStream), actix_web::Error> {
    handle_with_protocols(req, body, &[])
}

/// Begin handling websocket traffic with optional sub-protocol negotiation.
///
/// The first protocol offered by the client in the `Sec-WebSocket-Protocol` header that also
/// appears in `protocols` is returned in the handshake response.
///
/// If there is no overlap, no `Sec-WebSocket-Protocol` header is set in the response.
pub fn handle_with_protocols(
    req: &HttpRequest,
    body: web::Payload,
    protocols: &[&str],
) -> Result<(HttpResponse, Session, MessageStream), actix_web::Error> {
    let mut response = handshake_with_protocols(req, protocols)?;
    let (tx, rx) = channel(32);

    Ok((
        response
            .message_body(BodyStream::new(StreamingBody::new(rx)).boxed())?
            .into(),
        Session::new(tx),
        MessageStream::new(body.into_inner()),
    ))
}

fn handshake_with_protocols(
    req: &HttpRequest,
    protocols: &[&str],
) -> Result<actix_http::ResponseBuilder, actix_http::ws::HandshakeError> {
    let mut response = handshake(req.head())?;

    if let Some(protocol) = select_protocol(req, protocols) {
        response.insert_header((header::SEC_WEBSOCKET_PROTOCOL, protocol));
    }

    Ok(response)
}

fn select_protocol<'a>(req: &'a HttpRequest, protocols: &[&str]) -> Option<&'a str> {
    for requested_protocols in req.headers().get_all(header::SEC_WEBSOCKET_PROTOCOL) {
        let Ok(requested_protocols) = requested_protocols.to_str() else {
            continue;
        };

        for requested_protocol in requested_protocols.split(',').map(str::trim) {
            if requested_protocol.is_empty() {
                continue;
            }

            if protocols
                .iter()
                .any(|supported_protocol| *supported_protocol == requested_protocol)
            {
                return Some(requested_protocol);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use actix_web::{
        http::header::{self, HeaderValue},
        test::TestRequest,
        HttpRequest,
    };

    use super::handshake_with_protocols;

    fn ws_request(protocols: Option<&'static str>) -> HttpRequest {
        let mut req = TestRequest::default()
            .insert_header((header::UPGRADE, HeaderValue::from_static("websocket")))
            .insert_header((header::CONNECTION, HeaderValue::from_static("upgrade")))
            .insert_header((
                header::SEC_WEBSOCKET_VERSION,
                HeaderValue::from_static("13"),
            ))
            .insert_header((
                header::SEC_WEBSOCKET_KEY,
                HeaderValue::from_static("x3JJHMbDL1EzLkh9GBhXDw=="),
            ));

        if let Some(protocols) = protocols {
            req = req.insert_header((header::SEC_WEBSOCKET_PROTOCOL, protocols));
        }

        req.to_http_request()
    }

    #[test]
    fn handshake_selects_first_supported_client_protocol() {
        let req = ws_request(Some("p1,p2,p3"));

        let response = handshake_with_protocols(&req, &["p3", "p2"])
            .unwrap()
            .finish();

        assert_eq!(
            response.headers().get(header::SEC_WEBSOCKET_PROTOCOL),
            Some(&HeaderValue::from_static("p2")),
        );
    }

    #[test]
    fn handshake_omits_protocol_header_without_overlap() {
        let req = ws_request(Some("p1,p2,p3"));

        let response = handshake_with_protocols(&req, &["graphql"])
            .unwrap()
            .finish();

        assert!(response
            .headers()
            .get(header::SEC_WEBSOCKET_PROTOCOL)
            .is_none());
    }

    #[test]
    fn handshake_supports_multiple_protocol_headers() {
        let req = TestRequest::default()
            .insert_header((header::UPGRADE, HeaderValue::from_static("websocket")))
            .insert_header((header::CONNECTION, HeaderValue::from_static("upgrade")))
            .insert_header((
                header::SEC_WEBSOCKET_VERSION,
                HeaderValue::from_static("13"),
            ))
            .insert_header((
                header::SEC_WEBSOCKET_KEY,
                HeaderValue::from_static("x3JJHMbDL1EzLkh9GBhXDw=="),
            ))
            .append_header((header::SEC_WEBSOCKET_PROTOCOL, "p1"))
            .append_header((header::SEC_WEBSOCKET_PROTOCOL, "p2"))
            .to_http_request();

        let response = handshake_with_protocols(&req, &["p2"]).unwrap().finish();

        assert_eq!(
            response.headers().get(header::SEC_WEBSOCKET_PROTOCOL),
            Some(&HeaderValue::from_static("p2")),
        );
    }
}
