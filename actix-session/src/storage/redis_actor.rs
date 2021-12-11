use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::SessionStore;
use actix::Addr;
use actix_redis::{resp_array, RespValue};
use actix_redis::{Command, RedisActor};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use time::{self, Duration};

/// Use redis as session storage.
///
/// You need to pass the address of the redis server to the constructor.
pub struct RedisActorSessionStore {
    configuration: CacheConfiguration,
    addr: Addr<RedisActor>,
}

impl RedisActorSessionStore {
    pub fn builder<S: Into<String>>(connection_string: S) -> RedisActorSessionStoreBuilder {
        RedisActorSessionStoreBuilder {
            configuration: Default::default(),
            connection_string: connection_string.into(),
        }
    }

    pub fn new<S: Into<String>>(connection_string: S) -> RedisActorSessionStore {
        Self::builder(connection_string).build()
    }
}

struct CacheConfiguration {
    ttl: Duration,
    cache_keygen: Box<dyn Fn(&str) -> String>,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            ttl: time::Duration::seconds(7200),
            cache_keygen: Box::new(|s| s.to_owned()),
        }
    }
}

pub struct RedisActorSessionStoreBuilder {
    connection_string: String,
    configuration: CacheConfiguration,
}

impl RedisActorSessionStoreBuilder {
    /// Set time to live in seconds for session value.
    pub fn ttl(mut self, ttl: time::Duration) -> Self {
        self.configuration.ttl = ttl;
        self
    }

    /// Set a custom cache key generation strategy, expecting session key as input.
    pub fn cache_keygen(mut self, keygen: Box<dyn Fn(&str) -> String>) -> Self {
        self.configuration.cache_keygen = keygen;
        self
    }

    pub fn build(self) -> RedisActorSessionStore {
        RedisActorSessionStore {
            configuration: self.configuration,
            addr: RedisActor::start(self.connection_string),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl SessionStore for RedisActorSessionStore {
    async fn load(&self, session_key: &str) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key);
        let val = self
            .addr
            .send(Command(resp_array!["GET", cache_key]))
            .await
            .map_err(Into::into)
            .map_err(LoadError::GenericError)?
            .map_err(Into::into)
            .map_err(LoadError::GenericError)?;

        match val {
            RespValue::Error(e) => {
                return Err(LoadError::GenericError(anyhow::anyhow!(e)));
            }
            RespValue::SimpleString(s) => {
                if let Ok(val) = serde_json::from_str(&s) {
                    return Ok(Some(val));
                }
            }
            RespValue::BulkString(s) => {
                if let Ok(val) = serde_json::from_slice(&s) {
                    return Ok(Some(val));
                }
            }
            _ => {}
        }

        Ok(None)
    }

    async fn save(&self, session_state: SessionState) -> Result<String, SaveError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(SaveError::SerializationError)?;
        let session_key = generate_session_key();
        let cache_key = (self.configuration.cache_keygen)(&session_key);

        let cmd = Command(resp_array![
            "SET",
            cache_key,
            body,
            "NX",
            "EX",
            &format!("{}", self.configuration.ttl.whole_seconds())
        ]);

        let result = self
            .addr
            .send(cmd)
            .await
            .map_err(Into::into)
            .map_err(SaveError::GenericError)?
            .map_err(Into::into)
            .map_err(SaveError::GenericError)?;
        match result {
            RespValue::SimpleString(_) => Ok(session_key),
            RespValue::Nil => Err(SaveError::GenericError(anyhow::anyhow!(
                "Failed to save session state. A record with the same key already existed in Redis"
            ))),
            e => Err(SaveError::GenericError(anyhow::anyhow!(
                "Failed to save session state. {:?}",
                e
            ))),
        }
    }

    async fn update(
        &self,
        session_key: String,
        session_state: SessionState,
    ) -> Result<String, UpdateError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::SerializationError)?;
        let cache_key = (self.configuration.cache_keygen)(&session_key);

        let cmd = Command(resp_array![
            "SET",
            cache_key,
            body,
            "XX",
            "EX",
            &format!("{}", self.configuration.ttl.whole_seconds())
        ]);

        self.addr
            .send(cmd)
            .await
            .map_err(Into::into)
            .map_err(UpdateError::GenericError)?
            .map_err(Into::into)
            .map_err(UpdateError::GenericError)?;
        Ok(session_key)
    }

    async fn delete(&self, session_key: &str) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key);

        let res = self
            .addr
            .send(Command(resp_array!["DEL", cache_key]))
            .await?;

        match res {
            // Redis returns the number of deleted records
            Ok(RespValue::Integer(x)) if x > 0 => Ok(()),
            v => Err(anyhow::anyhow!(
                "Failed to remove session from cache. {:?}",
                v
            )),
        }
    }
}

// TODO: check if the current generation algorithm satisfies OWASP's recommendations
fn generate_session_key() -> String {
    let value = std::iter::repeat(())
        .map(|()| OsRng.sample(Alphanumeric))
        .take(32)
        .collect::<Vec<_>>();
    String::from_utf8(value).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::key;
    use crate::{Session, SessionMiddleware};
    use actix_web::{
        middleware, web,
        web::{get, post, resource},
        App, HttpResponse, Result,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct IndexResponse {
        user_id: Option<String>,
        counter: i32,
    }

    async fn index(session: Session) -> Result<HttpResponse> {
        let user_id: Option<String> = session.get::<String>("user_id").unwrap();
        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
    }

    async fn do_something(session: Session) -> Result<HttpResponse> {
        let user_id: Option<String> = session.get::<String>("user_id").unwrap();
        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .map_or(1, |inner| inner + 1);
        session.insert("counter", &counter)?;

        Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
    }

    #[derive(Deserialize)]
    struct Identity {
        user_id: String,
    }

    async fn login(user_id: web::Json<Identity>, session: Session) -> Result<HttpResponse> {
        let id = user_id.into_inner().user_id;
        session.insert("user_id", &id)?;
        session.renew();

        let counter: i32 = session
            .get::<i32>("counter")
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(HttpResponse::Ok().json(&IndexResponse {
            user_id: Some(id),
            counter,
        }))
    }

    async fn logout(session: Session) -> Result<HttpResponse> {
        let id: Option<String> = session.get("user_id")?;

        let body = if let Some(x) = id {
            session.purge();
            format!("Logged out: {}", x)
        } else {
            "Could not log out anonymous user".to_owned()
        };

        Ok(HttpResponse::Ok().body(body))
    }

    #[actix_rt::test]
    async fn test_session_workflow() {
        // Step 1:  GET index
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 2: POST to do_something
        //   - adds new session state in redis:  {"counter": 1}
        //   - set-cookie actix-session should be in response (session cookie #1)
        //   - response should be: {"counter": 1, "user_id": None}
        // Step 3:  GET index, including session cookie #1 in request
        //   - set-cookie will *not* be in response
        //   - response should be: {"counter": 1, "user_id": None}
        // Step 4: POST again to do_something, including session cookie #1 in request
        //   - updates session state in redis:  {"counter": 2}
        //   - response should be: {"counter": 2, "user_id": None}
        // Step 5: POST to login, including session cookie #1 in request
        //   - set-cookie actix-session will be in response  (session cookie #2)
        //   - updates session state in redis: {"counter": 2, "user_id": "ferris"}
        // Step 6: GET index, including session cookie #2 in request
        //   - response should be: {"counter": 2, "user_id": "ferris"}
        // Step 7: POST again to do_something, including session cookie #2 in request
        //   - updates session state in redis: {"counter": 3, "user_id": "ferris"}
        //   - response should be: {"counter": 3, "user_id": "ferris"}
        // Step 8: GET index, including session cookie #1 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        // Step 9: POST to logout, including session cookie #2
        //   - set-cookie actix-session will be in response with session cookie #2
        //     invalidation logic
        // Step 10: GET index, including session cookie #2 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}

        let srv = actix_test::start(|| {
            App::new()
                .wrap(
                    SessionMiddleware::builder(
                        RedisActorSessionStore::new("127.0.0.1:6379"),
                        key(),
                    )
                    .cookie_name("test-session".into())
                    .cookie_max_age(Some(time::Duration::days(7)))
                    .build(),
                )
                .wrap(middleware::Logger::default())
                .service(resource("/").route(get().to(index)))
                .service(resource("/do_something").route(post().to(do_something)))
                .service(resource("/login").route(post().to(login)))
                .service(resource("/logout").route(post().to(logout)))
        });

        // Step 1:  GET index
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_1a = srv.get("/").send();
        let mut resp_1 = req_1a.await.unwrap();
        assert!(resp_1.cookies().unwrap().is_empty());
        let result_1 = resp_1.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_1,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );

        // Step 2: POST to do_something
        //   - adds new session state in redis:  {"counter": 1}
        //   - set-cookie actix-session should be in response (session cookie #1)
        //   - response should be: {"counter": 1, "user_id": None}
        let req_2 = srv.post("/do_something").send();
        let mut resp_2 = req_2.await.unwrap();
        let result_2 = resp_2.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_2,
            IndexResponse {
                user_id: None,
                counter: 1
            }
        );
        let cookie_1 = resp_2
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_eq!(cookie_1.max_age(), Some(Duration::days(7)));

        // Step 3:  GET index, including session cookie #1 in request
        //   - set-cookie will *not* be in response
        //   - response should be: {"counter": 1, "user_id": None}
        let req_3 = srv.get("/").cookie(cookie_1.clone()).send();
        let mut resp_3 = req_3.await.unwrap();
        assert!(resp_3.cookies().unwrap().is_empty());
        let result_3 = resp_3.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_3,
            IndexResponse {
                user_id: None,
                counter: 1
            }
        );

        // Step 4: POST again to do_something, including session cookie #1 in request
        //   - updates session state in redis:  {"counter": 2}
        //   - response should be: {"counter": 2, "user_id": None}
        let req_4 = srv.post("/do_something").cookie(cookie_1.clone()).send();
        let mut resp_4 = req_4.await.unwrap();
        let result_4 = resp_4.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_4,
            IndexResponse {
                user_id: None,
                counter: 2
            }
        );

        // Step 5: POST to login, including session cookie #1 in request
        //   - set-cookie actix-session will be in response  (session cookie #2)
        //   - updates session state in redis: {"counter": 2, "user_id": "ferris"}
        let req_5 = srv
            .post("/login")
            .cookie(cookie_1.clone())
            .send_json(&json!({"user_id": "ferris"}));
        let mut resp_5 = req_5.await.unwrap();
        let cookie_2 = resp_5
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_ne!(cookie_1.value(), cookie_2.value());

        let result_5 = resp_5.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_5,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 2
            }
        );

        // Step 6: GET index, including session cookie #2 in request
        //   - response should be: {"counter": 2, "user_id": "ferris"}
        let req_6 = srv.get("/").cookie(cookie_2.clone()).send();
        let mut resp_6 = req_6.await.unwrap();
        let result_6 = resp_6.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_6,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 2
            }
        );

        // Step 7: POST again to do_something, including session cookie #2 in request
        //   - updates session state in redis: {"counter": 3, "user_id": "ferris"}
        //   - response should be: {"counter": 3, "user_id": "ferris"}
        let req_7 = srv.post("/do_something").cookie(cookie_2.clone()).send();
        let mut resp_7 = req_7.await.unwrap();
        let result_7 = resp_7.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_7,
            IndexResponse {
                user_id: Some("ferris".into()),
                counter: 3
            }
        );

        // Step 8: GET index, including session cookie #1 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_8 = srv.get("/").cookie(cookie_1.clone()).send();
        let mut resp_8 = req_8.await.unwrap();
        assert!(resp_8.cookies().unwrap().is_empty());
        let result_8 = resp_8.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_8,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );

        // Step 9: POST to logout, including session cookie #2
        //   - set-cookie actix-session will be in response with session cookie #2
        //     invalidation logic
        let req_9 = srv.post("/logout").cookie(cookie_2.clone()).send();
        let resp_9 = req_9.await.unwrap();
        let cookie_3 = resp_9
            .cookies()
            .unwrap()
            .clone()
            .into_iter()
            .find(|c| c.name() == "test-session")
            .unwrap();
        assert_eq!(0, cookie_3.max_age().map(|t| t.whole_seconds()).unwrap());

        // Step 10: GET index, including session cookie #2 in request
        //   - set-cookie actix-session should NOT be in response (session data is empty)
        //   - response should be: {"counter": 0, "user_id": None}
        let req_10 = srv.get("/").cookie(cookie_2.clone()).send();
        let mut resp_10 = req_10.await.unwrap();
        assert!(resp_10.cookies().unwrap().is_empty());
        let result_10 = resp_10.json::<IndexResponse>().await.unwrap();
        assert_eq!(
            result_10,
            IndexResponse {
                user_id: None,
                counter: 0
            }
        );
    }
}
