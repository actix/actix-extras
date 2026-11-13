//! Tests for the Reqwest 0.13 integration.

#![cfg(feature = "reqwest-0_13")]

use std::time::Duration;

use futures_util::TryStreamExt as _;
use reqwest_0_13::Client;
use russe::{
    Event, Message,
    reqwest_0_13::{Manager, ReqwestExt as _},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

async fn sse_server() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/events"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "id: 42\nevent: update\ndata: hello\n\n: heartbeat\n\n",
            russe::MEDIA_TYPE_STR,
        ))
        .expect(1)
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn response_stream_decodes_sse_events() {
    let server = sse_server().await;
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let events = client
        .get(format!("{}/events", server.uri()))
        .send()
        .await
        .unwrap()
        .sse_stream()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(
        events,
        vec![
            Event::Message(Message {
                data: "hello".into(),
                event: Some("update".into()),
                retry: None,
                id: Some("42".into()),
            }),
            Event::Comment("heartbeat".into()),
        ],
    );
}

#[tokio::test]
async fn manager_delivers_sse_events() {
    let server = sse_server().await;
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let req = client
        .get(format!("{}/events", server.uri()))
        .build()
        .unwrap();
    let mut manager = Manager::new(&client, req);

    let (task, events) = manager.send().await.unwrap();
    drop(manager);

    let events = tokio::time::timeout(
        Duration::from_secs(5),
        UnboundedReceiverStream::new(events).try_collect::<Vec<_>>(),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        events,
        vec![
            Event::Message(Message {
                data: "hello".into(),
                event: Some("update".into()),
                retry: None,
                id: Some("42".into()),
            }),
            Event::Comment("heartbeat".into()),
        ],
    );

    task.await.unwrap();
}
