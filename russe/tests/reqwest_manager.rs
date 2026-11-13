//! Tests for initial requests in both Reqwest integrations.

#![cfg(any(feature = "reqwest-0_12", feature = "reqwest-0_13"))]

macro_rules! manager_tests {
    ($reqwest:ident) => {
        mod $reqwest {
            use std::{error::Error as _, time::Duration};

            use ::$reqwest::Client;
            use indoc::indoc;
            use russe::{Error, Event, $reqwest::Manager};
            use tokio::{
                io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader},
                net::TcpListener,
                task::JoinHandle,
            };
            use wiremock::{
                Mock, MockServer, ResponseTemplate,
                matchers::{method, path},
            };

            async fn truncated_body_server() -> (String, JoinHandle<()>) {
                let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let url = format!("http://{}/events", listener.local_addr().unwrap());

                let task = tokio::spawn(async move {
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut socket = BufReader::new(socket);
                    let mut line = String::new();

                    loop {
                        line.clear();
                        assert_ne!(socket.read_line(&mut line).await.unwrap(), 0);

                        if line == "\r\n" {
                            break;
                        }
                    }

                    socket
                        .get_mut()
                        .write_all(indoc! {b"
                            HTTP/1.1 200 OK\r
                            Content-Type: text/event-stream\r
                            Content-Length: 1\r
                            Connection: close\r
                            \r
                        "})
                        .await
                        .unwrap();
                });

                (url, task)
            }

            fn client() -> Client {
                Client::builder()
                    .no_proxy()
                    .timeout(Duration::from_secs(1))
                    .build()
                    .unwrap()
            }

            #[tokio::test]
            async fn returns_initial_request_errors() {
                let server = MockServer::start().await;

                Mock::given(method("GET"))
                    .and(path("/events"))
                    .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
                    .expect(1)
                    .mount(&server)
                    .await;

                let client = client();
                let req = client
                    .get(format!("{}/events", server.uri()))
                    .build()
                    .unwrap();
                let mut manager = Manager::new(&client, req);

                let err = manager
                    .send()
                    .await
                    .expect_err("The initial request failure must return from send()");

                assert!(matches!(err, Error::Http { .. }), "got: {err:?}");
                assert!(
                    err.source()
                        .unwrap()
                        .downcast_ref::<::$reqwest::Error>()
                        .unwrap()
                        .is_timeout()
                );
            }

            #[tokio::test]
            async fn retries_after_initial_request_error() {
                let server = MockServer::start().await;

                let slow_response = Mock::given(method("GET"))
                    .and(path("/events"))
                    .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
                    .expect(1)
                    .mount_as_scoped(&server)
                    .await;

                let client = client();
                let req = client
                    .get(format!("{}/events", server.uri()))
                    .build()
                    .unwrap();
                let mut manager = Manager::new(&client, req);

                manager.send().await.unwrap_err();

                drop(slow_response);

                Mock::given(method("GET"))
                    .and(path("/events"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_raw(": ping\n\n", russe::MEDIA_TYPE_STR),
                    )
                    .expect(1)
                    .mount(&server)
                    .await;

                let (task, mut events) = manager.send().await.unwrap();
                let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();

                assert_eq!(event, Event::Comment("ping".into()));

                task.await.unwrap();
            }

            #[tokio::test]
            async fn sends_body_errors_through_event_receiver() {
                let (url, server) = truncated_body_server().await;
                let client = client();
                let req = client.get(url).build().unwrap();
                let mut manager = Manager::new(&client, req);

                let (task, mut events) = manager.send().await.unwrap();
                let err = tokio::time::timeout(Duration::from_secs(5), events.recv())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap_err();

                assert!(matches!(err, Error::Io { .. }), "got: {err:?}");

                task.await.unwrap();
                server.await.unwrap();
            }
        }
    };
}

#[cfg(feature = "reqwest-0_12")]
manager_tests!(reqwest_0_12);

#[cfg(feature = "reqwest-0_13")]
manager_tests!(reqwest_0_13);
