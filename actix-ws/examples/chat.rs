use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use actix_web::{
    middleware::Logger, web, web::Html, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_ws::{Message, Session};
use bytestring::ByteString;
use futures_util::{stream::FuturesUnordered, StreamExt as _};
use tokio::sync::Mutex;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct Chat {
    inner: Arc<Mutex<ChatInner>>,
}

struct ChatInner {
    sessions: Vec<Session>,
}

impl Chat {
    fn new() -> Self {
        Chat {
            inner: Arc::new(Mutex::new(ChatInner {
                sessions: Vec::new(),
            })),
        }
    }

    async fn insert(&self, session: Session) {
        self.inner.lock().await.sessions.push(session);
    }

    async fn send(&self, msg: impl Into<ByteString>) {
        let msg = msg.into();

        let mut inner = self.inner.lock().await;
        let mut unordered = FuturesUnordered::new();

        for mut session in inner.sessions.drain(..) {
            let msg = msg.clone();

            unordered.push(async move {
                let res = session.text(msg).await;
                res.map(|_| session)
                    .map_err(|_| tracing::debug!("Dropping session"))
            });
        }

        while let Some(res) = unordered.next().await {
            if let Ok(session) = res {
                inner.sessions.push(session);
            }
        }
    }
}

async fn ws(
    req: HttpRequest,
    body: web::Payload,
    chat: web::Data<Chat>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut stream) = actix_ws::handle(&req, body)?;

    chat.insert(session.clone()).await;
    tracing::info!("Inserted session");

    let alive = Arc::new(Mutex::new(Instant::now()));

    let mut session2 = session.clone();
    let alive2 = alive.clone();
    actix_web::rt::spawn(async move {
        let mut interval = actix_web::rt::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            if session2.ping(b"").await.is_err() {
                break;
            }

            if Instant::now().duration_since(*alive2.lock().await) > Duration::from_secs(10) {
                let _ = session2.close(None).await;
                break;
            }
        }
    });

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        return;
                    }
                }
                Message::Text(msg) => {
                    tracing::info!("Relaying msg: {msg}");
                    chat.send(msg).await;
                }
                Message::Close(reason) => {
                    let _ = session.close(reason).await;
                    tracing::info!("Got close, bailing");
                    return;
                }
                Message::Continuation(_) => {
                    let _ = session.close(None).await;
                    tracing::info!("Got continuation, bailing");
                    return;
                }
                Message::Pong(_) => {
                    *alive.lock().await = Instant::now();
                }
                _ => (),
            };
        }
        let _ = session.close(None).await;
    });
    tracing::info!("Spawned");

    Ok(response)
}

async fn index() -> impl Responder {
    Html::new(include_str!("chat.html").to_owned())
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

    let chat = Chat::new();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(chat.clone()))
            .route("/", web::get().to(index))
            .route("/ws", web::get().to(ws))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}
