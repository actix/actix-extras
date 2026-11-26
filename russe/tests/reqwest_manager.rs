//! Tests for connection managers in both Reqwest integrations.

#![cfg(any(feature = "reqwest-0_12", feature = "reqwest-0_13"))]

macro_rules! manager_tests {
    ($reqwest:ident) => {
        mod $reqwest {
            use std::{error::Error as _, time::Duration};

            use ::$reqwest::Client;
            use indoc::indoc;
            use russe::{
                DecodeError, Event,
                $reqwest::{Manager, ManagerError},
            };
            use tokio::{
                io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader},
                net::TcpListener,
                task::JoinHandle,
            };
            use tokio_util::time::FutureExt as _;
            use wiremock::{
                Mock, MockServer, ResponseTemplate,
                matchers::{method, path},
            };

            const TRUNCATED: &[u8] = indoc! {b"
                HTTP/1.1 200 OK\r
                Content-Type: text/event-stream\r
                Content-Length: 1\r
                Connection: close\r
                \r
            "};

            const COMPLETE: &[u8] = indoc! {b"
                HTTP/1.1 200 OK\r
                Content-Type: text/event-stream\r
                Content-Length: 8\r
                Connection: close\r
                \r
                : ping

            "};

            async fn response_server(
                responses: &'static [&'static [u8]],
            ) -> (String, JoinHandle<()>) {
                let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let url = format!("http://{}/events", listener.local_addr().unwrap());

                let task = tokio::spawn(async move {
                    for res in responses {
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

                        socket.get_mut().write_all(res).await.unwrap();
                    }
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
            async fn reconnect_uses_committed_id_and_server_delay() {
                let server = MockServer::start().await;

                Mock::given(method("GET"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_raw("retry: 1\n\ndata: hello\n\n", russe::MEDIA_TYPE_STR),
                    )
                    .up_to_n_times(1)
                    .expect(1)
                    .mount(&server)
                    .await;

                Mock::given(method("GET"))
                    .and(wiremock::matchers::header("last-event-id", "42"))
                    .respond_with(ResponseTemplate::new(204))
                    .expect(1)
                    .mount(&server)
                    .await;

                let client = client();
                let req = client.get(server.uri()).build().unwrap();

                let mut manager = Manager::new(&client, req).reconnect(russe::Reconnect::OnEof {
                    delay: Duration::from_secs(60),
                    max_retries: 3,
                });
                manager.commit_id("42");

                let (task, mut events) = manager.send().await.unwrap();
                task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();

                assert!(matches!(events.recv().await, Some(Ok(Event::Retry(_)))));
                assert!(matches!(events.recv().await, Some(Ok(Event::Message(_)))));
                assert!(events.recv().await.is_none());

                let requests = server.received_requests().await.unwrap();
                assert_eq!(requests[0].headers.get("last-event-id").unwrap(), "42");
            }

            #[tokio::test]
            async fn on_error_reconnects_after_truncation_and_stops_at_clean_eof() {
                let (url, server) = response_server(&[TRUNCATED, COMPLETE]).await;

                let client = client();
                let req = client.get(url).build().unwrap();

                let mut manager = Manager::new(&client, req).reconnect(russe::Reconnect::OnError {
                    delay: Duration::from_millis(1),
                    max_retries: 3,
                });

                let (task, mut events) = manager.send().await.unwrap();
                task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();

                assert!(matches!(
                    events.recv().await,
                    Some(Err(ManagerError::Decode {
                        source: DecodeError::Io { .. }
                    }))
                ));
                assert_eq!(
                    events.recv().await.unwrap().unwrap(),
                    Event::Comment("ping".into())
                );
                assert!(events.recv().await.is_none());

                server.await.unwrap();
            }

            #[tokio::test]
            async fn always_reports_errors_and_reconnects_after_clean_eof() {
                let (url, server) = response_server(&[TRUNCATED, COMPLETE, COMPLETE]).await;

                let client = client();
                let req = client.get(url).build().unwrap();

                let mut manager = Manager::new(&client, req).reconnect(russe::Reconnect::Always {
                    delay: Duration::ZERO,
                    max_retries: 2,
                });

                let (task, mut events) = manager.send().await.unwrap();
                task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();

                assert!(matches!(
                    events.recv().await,
                    Some(Err(ManagerError::Decode {
                        source: DecodeError::Io { .. }
                    }))
                ));

                for _ in 0..2 {
                    assert_eq!(
                        events.recv().await.unwrap().unwrap(),
                        Event::Comment("ping".into())
                    );
                }

                assert!(events.recv().await.is_none());

                server.await.unwrap();
            }

            #[tokio::test]
            async fn on_eof_reports_truncation_and_closes() {
                let (url, server) = response_server(&[TRUNCATED]).await;

                let client = client();
                let req = client.get(url).build().unwrap();

                let mut manager = Manager::new(&client, req).reconnect(russe::Reconnect::OnEof {
                    delay: Duration::ZERO,
                    max_retries: 1,
                });

                let (task, mut events) = manager.send().await.unwrap();
                task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();

                assert!(matches!(
                    events.recv().await,
                    Some(Err(ManagerError::Decode {
                        source: DecodeError::Io { .. }
                    }))
                ));
                assert!(events.recv().await.is_none());

                server.await.unwrap();
            }

            #[tokio::test]
            async fn on_error_counts_successful_and_failed_reconnect_requests() {
                let (url, server) = response_server(&[TRUNCATED, TRUNCATED]).await;

                let client = client();
                let req = client.get(url).build().unwrap();

                let mut manager = Manager::new(&client, req).reconnect(russe::Reconnect::OnError {
                    delay: Duration::ZERO,
                    max_retries: 2,
                });

                let (task, mut events) = manager.send().await.unwrap();
                task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();
                server.await.unwrap();

                // Both responses were truncated. The final request found the listener closed.
                assert!(matches!(
                    events.recv().await,
                    Some(Err(ManagerError::Decode {
                        source: DecodeError::Io { .. }
                    }))
                ));
                assert!(matches!(
                    events.recv().await,
                    Some(Err(ManagerError::Decode {
                        source: DecodeError::Io { .. }
                    }))
                ));
                assert!(matches!(
                    events.recv().await,
                    Some(Err(ManagerError::Http { .. }))
                ));
                assert!(events.recv().await.is_none());
            }

            #[tokio::test]
            async fn on_eof_stops_at_max_retries() {
                for max_retries in [0, 2] {
                    let server = MockServer::start().await;

                    Mock::given(method("GET"))
                        .respond_with(ResponseTemplate::new(200))
                        .expect(1 + max_retries as u64)
                        .mount(&server)
                        .await;

                    let client = client();
                    let req = client.get(server.uri()).build().unwrap();

                    let mut manager =
                        Manager::new(&client, req).reconnect(russe::Reconnect::OnEof {
                            delay: Duration::ZERO,
                            max_retries,
                        });

                    let (task, mut events) = manager.send().await.unwrap();
                    task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();

                    assert!(events.recv().await.is_none());
                }
            }

            #[tokio::test]
            async fn sends_accept_header() {
                let server = MockServer::start().await;

                Mock::given(method("GET"))
                    .respond_with(ResponseTemplate::new(200))
                    .mount(&server)
                    .await;

                let client = client();
                let url = server.uri();
                let custom = "text/event-stream, text/plain;q=0.1";

                for req in [client.get(&url), client.get(&url).header("accept", custom)] {
                    let mut manager = Manager::new(&client, req.build().unwrap());

                    let (task, _) = manager.send().await.unwrap();
                    task.await.unwrap();
                }

                let requests = server.received_requests().await.unwrap();
                assert_eq!(requests[0].headers["accept"], russe::MEDIA_TYPE_STR);
                assert_eq!(requests[1].headers["accept"], custom);
            }

            #[tokio::test]
            async fn rejects_invalid_event_id_before_sending() {
                let client = client();
                let req = client.get("http://localhost/events").build().unwrap();
                let mut manager = Manager::new(&client, req);
                manager.commit_id("first\nsecond");

                assert!(matches!(
                    manager.send().await,
                    Err(ManagerError::InvalidEventId { .. })
                ));
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

                assert!(matches!(err, ManagerError::Http { .. }), "got: {err:?}");
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
                let event = events
                    .recv()
                    .timeout(Duration::from_secs(5))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();

                assert_eq!(event, Event::Comment("ping".into()));

                task.await.unwrap();

                assert!(events.is_closed());
                assert!(events.recv().await.is_none());
            }

            #[tokio::test]
            async fn sends_body_errors_through_event_receiver() {
                let (url, server) = response_server(&[TRUNCATED]).await;

                let client = client();
                let req = client.get(url).build().unwrap();

                let mut manager = Manager::new(&client, req);

                let (task, mut events) = manager.send().await.unwrap();
                let err = events
                    .recv()
                    .timeout(Duration::from_secs(5))
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap_err();

                assert!(
                    matches!(
                        err,
                        ManagerError::Decode {
                            source: DecodeError::Io { .. }
                        }
                    ),
                    "got: {err:?}"
                );

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
