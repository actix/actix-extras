use std::io;

use actix_web::{middleware::Logger, web, App, HttpRequest, HttpServer, Responder};
use actix_ws::codec::{CodecMessage, CodecStreamError, JsonCodec};
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Serialize, Deserialize)]
enum ChatMsg {
    Say { text: String },
}

async fn ws(req: HttpRequest, body: web::Payload) -> actix_web::Result<impl Responder> {
    let (response, session, msg_stream) = actix_ws::handle(&req, body)?;

    let mut session = session.with_codec::<ChatMsg, _>(JsonCodec::default());
    let mut msg_stream = msg_stream.with_codec::<ChatMsg, _>(JsonCodec::default());

    actix_web::rt::spawn(async move {
        while let Some(item) = msg_stream.recv().await {
            match item {
                Ok(CodecMessage::Item(ChatMsg::Say { text })) => {
                    // echo back a structured message
                    if session.send(&ChatMsg::Say { text }).await.is_err() {
                        return;
                    }
                }

                Ok(CodecMessage::Ping(bytes)) => {
                    let _ = session.session_mut().pong(&bytes).await;
                }

                Ok(CodecMessage::Pong(_)) => {}

                Ok(CodecMessage::Close(reason)) => {
                    let _ = session.close(reason).await;
                    return;
                }

                Err(CodecStreamError::Codec(err)) => {
                    // invalid JSON payload or schema mismatch
                    tracing::warn!("invalid JSON payload: {err}");
                }

                Err(CodecStreamError::Protocol(err)) => {
                    tracing::warn!("websocket protocol error: {err}");
                    return;
                }
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/ws", web::get().to(ws))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}
