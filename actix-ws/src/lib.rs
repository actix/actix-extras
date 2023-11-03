//! WebSockets for Actix Web, without actors.
//!
//! For usage, see documentation on [`handle()`].

#![deny(rust_2018_idioms, nonstandard_style, future_incompatible)]
#![warn(missing_docs)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub use actix_http::ws::{CloseCode, CloseReason, Message, ProtocolError};
use actix_http::{
    body::{BodyStream, MessageBody},
    ws::handshake,
};
use actix_web::{web, HttpRequest, HttpResponse};
use tokio::sync::mpsc::channel;

mod fut;
mod session;

pub use self::{
    fut::{MessageStream, StreamingBody},
    session::{Closed, Session},
};

/// Begin handling websocket traffic
///
/// ```no_run
/// use actix_web::{middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer};
/// use actix_ws::Message;
/// use futures::stream::StreamExt as _;
///
/// async fn ws(req: HttpRequest, body: web::Payload) -> Result<HttpResponse, Error> {
///     let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;
///
///     actix_rt::spawn(async move {
///         while let Some(Ok(msg)) = msg_stream.next().await {
///             match msg {
///                 Message::Ping(bytes) => {
///                     if session.pong(&bytes).await.is_err() {
///                         return;
///                     }
///                 }
///                 Message::Text(s) => println!("Got text, {}", s),
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
/// #[actix_rt::main]
/// async fn main() -> Result<(), anyhow::Error> {
///     HttpServer::new(move || {
///         App::new()
///             .wrap(Logger::default())
///             .route("/ws", web::get().to(ws))
///     })
///     .bind("127.0.0.1:8080")?
///     .run()
///     .await?;
///
///     Ok(())
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
