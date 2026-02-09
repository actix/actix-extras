//! WebSockets for Actix Web, without actors.
//!
//! For usage, see documentation on [`handle()`].

#![warn(missing_docs)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use actix_http::ws::{CloseCode, CloseReason, Item, Message, ProtocolError};
use actix_http::{
    body::{BodyStream, MessageBody},
    ws::handshake,
};
use actix_web::{web, HttpRequest, HttpResponse};
use tokio::sync::mpsc::channel;

mod aggregated;
mod session;
mod stream;

pub use self::{
    aggregated::{AggregatedMessage, AggregatedMessageStream},
    session::{Closed, Session},
    stream::{MessageStream, StreamingBody},
};

/// Begin handling websocket traffic
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
    let mut response = handshake(req.head())?;
    let (tx, rx) = channel(32);

    Ok((
        response
            .message_body(BodyStream::new(StreamingBody::new(rx)).boxed())?
            .into(),
        Session::new(tx),
        MessageStream::new(body.into_inner()),
    ))
}
