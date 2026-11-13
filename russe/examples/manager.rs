//! Demonstrates usage of the SSE connection manager.

extern crate reqwest_0_12 as reqwest;

use futures_util::StreamExt as _;
use reqwest::{Method, Request};
use russe::reqwest_0_12::Manager;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[tokio::main(flavor = "current_thread")]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let client = reqwest::Client::default();

    let mut req = Request::new(Method::GET, "https://sse.dev/test".parse().unwrap());
    let headers = req.headers_mut();
    headers.insert("accept", russe::MEDIA_TYPE_STR.parse().unwrap());

    let mut manager = Manager::new(&client, req);

    let (_task_handle, events) = manager.send().await?;

    let mut event_stream = UnboundedReceiverStream::new(events);

    while let Some(Ok(ev)) = event_stream.next().await {
        println!("{ev:?}");

        if let russe::Event::Message(msg) = ev
            && let Some(id) = msg.id
        {
            manager.commit_id(id);
        }
    }

    Ok(())
}
