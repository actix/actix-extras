use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use actix_web::{middleware::Logger, web, App, HttpRequest, HttpResponse, HttpServer};
use actix_ws::{Message, Session};
use futures::stream::{FuturesUnordered, StreamExt as _};
use log::info;
use tokio::sync::Mutex;

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

    async fn send(&self, msg: String) {
        let mut inner = self.inner.lock().await;
        let mut unordered = FuturesUnordered::new();

        for mut session in inner.sessions.drain(..) {
            let msg = msg.clone();
            unordered.push(async move {
                let res = session.text(msg).await;
                res.map(|_| session).map_err(|_| info!("Dropping session"))
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
    info!("Inserted session");

    let alive = Arc::new(Mutex::new(Instant::now()));

    let mut session2 = session.clone();
    let alive2 = alive.clone();
    actix_rt::spawn(async move {
        let mut interval = actix_rt::time::interval(Duration::from_secs(5));
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

    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        return;
                    }
                }
                Message::Text(s) => {
                    info!("Relaying text, {}", s);
                    let s: &str = s.as_ref();
                    chat.send(s.into()).await;
                }
                Message::Close(reason) => {
                    let _ = session.close(reason).await;
                    info!("Got close, bailing");
                    return;
                }
                Message::Continuation(_) => {
                    let _ = session.close(None).await;
                    info!("Got continuation, bailing");
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
    info!("Spawned");

    Ok(response)
}

async fn index() -> HttpResponse {
    let s = r#"
<html>
    <head>
        <meta charset="utf-8" />
        <title>Chat</title>
        <script>
function onLoad() {
    console.log("BOOTING");
    const socket = new WebSocket("ws://localhost:8080/ws");
    const input = document.getElementById("chat-input");
    const logs = document.getElementById("chat-logs");

    if (!input || !logs) {
        alert("Couldn't find required elements");
        console.err("Couldn't find required elements");
        return;
    }

    input.addEventListener("keyup", event => {
        if (event.isComposing) {
            return;
        }
        if (event.key != "Enter") {
            return;
        }

        socket.send(input.value);
        input.value = "";
    }, false);

    socket.onmessage = event => {
        const newNode = document.createElement("li");
        newNode.textContent = event.data;

        let firstChild = null;
        for (const n of logs.childNodes.values()) {
            if (n.nodeType == 1) {
                firstChild = n;
                break;
            }
        }

        if (firstChild) {
            logs.insertBefore(newNode, firstChild);
        } else {
            logs.appendChild(newNode);
        }
    };

    window.addEventListener("beforeunload", () => { socket.close() });
}

if (document.readyState === "complete") {
    onLoad();
} else {
    document.addEventListener("DOMContentLoaded", onLoad, false);
}
        </script>
    </head>
    <body>
        <input id="chat-input" type="test" />
        <ul id="chat-logs">
        </ul>
    </body>
</html>
    "#;

    HttpResponse::Ok().content_type("text/html").body(s)
}

#[actix_rt::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();
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
