use crate::fixtures::user_id;
use crate::test_app::TestApp;
use actix_web::http::StatusCode;

#[actix_web::test]
async fn opaque_401_is_returned_for_unauthenticated_users() {
    let app = TestApp::spawn();

    let response = app.get_identity_required().await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.bytes().await.unwrap().is_empty());
}

#[actix_web::test]
async fn login_works() {
    let app = TestApp::spawn();
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));

    // Access identity-restricted route successfully
    let response = app.get_identity_required().await;
    assert!(response.error_for_status().is_ok());
}

// pub(super) async fn complex_workflow<F, Store>(
//     store_builder: F,
//     is_invalidation_supported: bool,
//     policy: CookieContentSecurity,
// ) where
//     Store: SessionStore + 'static,
//     F: Fn() -> Store + Clone + Send + 'static,
// {
//     let session_ttl = time::Duration::days(7);
//     let srv = actix_test::start(move || {
//         App::new()
//             .wrap(
//                 SessionMiddleware::builder(store_builder(), key())
//                     .cookie_name("test-session".into())
//                     .cookie_content_security(policy)
//                     .session_lifecycle(PersistentSession::default().session_ttl(session_ttl))
//                     .build(),
//             )
//             .wrap(middleware::Logger::default())
//             .service(resource("/").route(get().to(index)))
//             .service(resource("/do_something").route(post().to(do_something)))
//             .service(resource("/login").route(post().to(login)))
//             .service(resource("/logout").route(post().to(logout)))
//     });
//
//     // Step 1:  GET index
//     //   - set-cookie actix-session should NOT be in response (session data is empty)
//     //   - response should be: {"counter": 0, "user_id": None}
//     let req_1a = srv.get("/").send();
//     let mut resp_1 = req_1a.await.unwrap();
//     assert!(resp_1.cookies().unwrap().is_empty());
//     let result_1 = resp_1.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_1,
//         IndexResponse {
//             user_id: None,
//             counter: 0
//         }
//     );
//
//     // Step 2: POST to do_something
//     //   - adds new session state in redis:  {"counter": 1}
//     //   - set-cookie actix-session should be in response (session cookie #1)
//     //   - response should be: {"counter": 1, "user_id": None}
//     let req_2 = srv.post("/do_something").send();
//     let mut resp_2 = req_2.await.unwrap();
//     let result_2 = resp_2.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_2,
//         IndexResponse {
//             user_id: None,
//             counter: 1
//         }
//     );
//     let cookie_1 = resp_2
//         .cookies()
//         .unwrap()
//         .clone()
//         .into_iter()
//         .find(|c| c.name() == "test-session")
//         .unwrap();
//     assert_eq!(cookie_1.max_age(), Some(session_ttl));
//
//     // Step 3:  GET index, including session cookie #1 in request
//     //   - set-cookie will *not* be in response
//     //   - response should be: {"counter": 1, "user_id": None}
//     let req_3 = srv.get("/").cookie(cookie_1.clone()).send();
//     let mut resp_3 = req_3.await.unwrap();
//     assert!(resp_3.cookies().unwrap().is_empty());
//     let result_3 = resp_3.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_3,
//         IndexResponse {
//             user_id: None,
//             counter: 1
//         }
//     );
//
//     // Step 4: POST again to do_something, including session cookie #1 in request
//     //   - set-cookie will be in response (session cookie #2)
//     //   - updates session state:  {"counter": 2}
//     //   - response should be: {"counter": 2, "user_id": None}
//     let req_4 = srv.post("/do_something").cookie(cookie_1.clone()).send();
//     let mut resp_4 = req_4.await.unwrap();
//     let result_4 = resp_4.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_4,
//         IndexResponse {
//             user_id: None,
//             counter: 2
//         }
//     );
//     let cookie_2 = resp_4
//         .cookies()
//         .unwrap()
//         .clone()
//         .into_iter()
//         .find(|c| c.name() == "test-session")
//         .unwrap();
//     assert_eq!(cookie_2.max_age(), cookie_1.max_age());
//
//     // Step 5: POST to login, including session cookie #2 in request
//     //   - set-cookie actix-session will be in response  (session cookie #3)
//     //   - updates session state: {"counter": 2, "user_id": "ferris"}
//     let req_5 = srv
//         .post("/login")
//         .cookie(cookie_2.clone())
//         .send_json(&json!({"user_id": "ferris"}));
//     let mut resp_5 = req_5.await.unwrap();
//     let cookie_3 = resp_5
//         .cookies()
//         .unwrap()
//         .clone()
//         .into_iter()
//         .find(|c| c.name() == "test-session")
//         .unwrap();
//     assert_ne!(cookie_2.value(), cookie_3.value());
//
//     let result_5 = resp_5.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_5,
//         IndexResponse {
//             user_id: Some("ferris".into()),
//             counter: 2
//         }
//     );
//
//     // Step 6: GET index, including session cookie #3 in request
//     //   - response should be: {"counter": 2, "user_id": "ferris"}
//     let req_6 = srv.get("/").cookie(cookie_3.clone()).send();
//     let mut resp_6 = req_6.await.unwrap();
//     let result_6 = resp_6.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_6,
//         IndexResponse {
//             user_id: Some("ferris".into()),
//             counter: 2
//         }
//     );
//
//     // Step 7: POST again to do_something, including session cookie #3 in request
//     //   - updates session state: {"counter": 3, "user_id": "ferris"}
//     //   - response should be: {"counter": 3, "user_id": "ferris"}
//     let req_7 = srv.post("/do_something").cookie(cookie_3.clone()).send();
//     let mut resp_7 = req_7.await.unwrap();
//     let result_7 = resp_7.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_7,
//         IndexResponse {
//             user_id: Some("ferris".into()),
//             counter: 3
//         }
//     );
//
//     // Step 8: GET index, including session cookie #2 in request
//     // If invalidation is supported, no state will be found associated to this session.
//     // If invalidation is not supported, the old state will still be retrieved.
//     let req_8 = srv.get("/").cookie(cookie_2.clone()).send();
//     let mut resp_8 = req_8.await.unwrap();
//     if is_invalidation_supported {
//         assert!(resp_8.cookies().unwrap().is_empty());
//         let result_8 = resp_8.json::<IndexResponse>().await.unwrap();
//         assert_eq!(
//             result_8,
//             IndexResponse {
//                 user_id: None,
//                 counter: 0
//             }
//         );
//     } else {
//         let result_8 = resp_8.json::<IndexResponse>().await.unwrap();
//         assert_eq!(
//             result_8,
//             IndexResponse {
//                 user_id: None,
//                 counter: 2
//             }
//         );
//     }
//
//     // Step 9: POST to logout, including session cookie #3
//     //   - set-cookie actix-session will be in response with session cookie #3
//     //     invalidation logic
//     let req_9 = srv.post("/logout").cookie(cookie_3.clone()).send();
//     let resp_9 = req_9.await.unwrap();
//     let cookie_3 = resp_9
//         .cookies()
//         .unwrap()
//         .clone()
//         .into_iter()
//         .find(|c| c.name() == "test-session")
//         .unwrap();
//     assert_eq!(0, cookie_3.max_age().map(|t| t.whole_seconds()).unwrap());
//     assert_eq!("/", cookie_3.path().unwrap());
//
//     // Step 10: GET index, including session cookie #3 in request
//     //   - set-cookie actix-session should NOT be in response if invalidation is supported
//     //   - response should be: {"counter": 0, "user_id": None}
//     let req_10 = srv.get("/").cookie(cookie_3.clone()).send();
//     let mut resp_10 = req_10.await.unwrap();
//     if is_invalidation_supported {
//         assert!(resp_10.cookies().unwrap().is_empty());
//     }
//     let result_10 = resp_10.json::<IndexResponse>().await.unwrap();
//     assert_eq!(
//         result_10,
//         IndexResponse {
//             user_id: None,
//             counter: 0
//         }
//     );
// }
//
