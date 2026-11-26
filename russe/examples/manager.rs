//! Demonstrates usage of the SSE connection manager.

extern crate reqwest_0_13 as reqwest;

use russe::{Event, reqwest_0_13::Manager};

#[tokio::main(flavor = "local")]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let client = reqwest::Client::default();

    let mut req = client.get("https://httpbin.org/sse").build()?;
    let headers = req.headers_mut();
    headers.insert("accept", russe::MEDIA_TYPE_STR.parse().unwrap());

    let mut manager = Manager::new(&client, req);

    let (_task_handle, mut events) = manager.send().await?;

    while let Some(Ok(ev)) = events.recv().await {
        if let Event::Message(msg) = ev {
            println!("{}", msg.data);

            if let Some(id) = msg.id {
                manager.commit_id(id);
            }
        }
    }

    Ok(())
}
