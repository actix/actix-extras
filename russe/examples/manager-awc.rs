//! Demonstrates usage of the SSE connection manager with AWC.

extern crate awc_3 as awc;

use std::time::Duration;

use russe::{Event, Reconnect, awc_3::Manager};

#[tokio::main(flavor = "local")]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let client = awc::Client::default();

    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://httpbin.org/sse".to_owned());
    let req = client.get(url);

    let mut manager = Manager::new(req).reconnect(Reconnect::OnError {
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
