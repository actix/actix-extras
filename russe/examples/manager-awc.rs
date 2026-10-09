//! Demonstrates usage of the SSE connection manager with AWC.

extern crate awc_3 as awc;

use russe::{Event, awc_3::Manager};

#[tokio::main(flavor = "local")]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let client = awc::Client::default();

    let req = client.get("http://httpbin.org/sse");

    let mut manager = Manager::new(req);

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
