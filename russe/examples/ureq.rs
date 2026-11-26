//! Reads SSE messages on a worker thread and prints their data.
//!
//! Run with `cargo run -p russe --example ureq --features ureq-3`.

extern crate ureq_3 as ureq;

use std::thread;

use eyre::eyre;
use russe::{Event, MEDIA_TYPE_STR, ureq_3::UreqExt as _};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let events = ureq::get("http://httpbin.org/sse")
        .header("Accept", MEDIA_TYPE_STR)
        .call()?
        .sse_iter();

    let reader = thread::spawn(move || -> russe::Result<()> {
        for event in events {
            if let Event::Message(msg) = event? {
                println!("{}", msg.data);
            }
        }

        Ok(())
    });

    reader
        .join()
        .map_err(|_| eyre!("SSE reader thread panicked"))??;

    Ok(())
}
