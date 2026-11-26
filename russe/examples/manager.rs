//! Demonstrates usage of the SSE connection manager.

extern crate reqwest_0_13 as reqwest;

use std::time::Duration;

use russe::{Event, Reconnect, reqwest_0_13::Manager};

#[tokio::main(flavor = "local")]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let client = reqwest::Client::default();

    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://httpbin.org/sse".to_owned());
    let req = client.get(url).build()?;

    let mut manager = Manager::new(&client, req).reconnect(Reconnect::OnError {
        delay: Duration::from_secs(1),
        max_retries: 3,
    });

    let (_task_handle, mut events) = manager.send().await?;

    while let Some(event) = events.recv().await {
        match event {
            Ok(Event::Message(msg)) => {
                println!("{}", msg.data);

                if let Some(id) = msg.id {
                    manager.commit_id(id);
                }
            }

            Ok(_) => {
                // ignore comments & retry frames
            }

            Err(err) => eprintln!("{}", err_report::Report::new(err)),
        }
    }

    Ok(())
}
