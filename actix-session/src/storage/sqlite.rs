use std::{collections::HashMap, sync::Arc};

use actix_web::cookie::time::{Duration, OffsetDateTime};
use anyhow::Error;
use r2d2::Pool;
use r2d2_sqlite::{self, SqliteConnectionManager};
use rusqlite::params;

use super::{
    interface::{LoadError, SaveError, SessionState, UpdateError},
    utils::generate_session_key,
    SessionKey, SessionStore,
};

/// Use Sqlite as session storage backend.
///
/// ```no_run
/// use actix_session::{storage::SqliteSessionStore, Session, SessionMiddleware};
/// use actix_web::{
///   cookie::{time::Duration, Key},
///   middleware, web, App, Error, HttpRequest, HttpServer, Responder,
/// };
/// use r2d2_sqlite::{self, SqliteConnectionManager};
///
/// // The secret key would usually be read from a configuration file/environment variables.
/// fn get_secret_key() -> Key {
///     # todo!()
///     // [...]
/// }
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///   let secret_key = get_secret_key();
///
///   let manager = SqliteConnectionManager::file("sessions.db");
///   let pool = r2d2::Pool::<r2d2_sqlite::SqliteConnectionManager>::new(manager).unwrap();
///   let store = SqliteSessionStore::new(pool, true).unwrap();
///
///   HttpServer::new(move || {
///       App::new()
///           .wrap(SessionMiddleware::new(
///                 store.clone(),
///                 secret_key.clone()
///            ))
///           .default_service(web::to(|| HttpResponse::Ok())))
///   })
///   .bind(("127.0.0.1", 8080))?
///   .run()
///   .await
/// }
/// ```
///
/// # Implementation notes
/// `SqliteSessionStore` uses rusqlite, r2d2 and r2d2_sqlite.
///
/// [`rusqlite`]: https://github.com/rusqlite/rusqlite
/// [`r2d2`]: https://github.com/sfackler/r2d2
/// [`r2d2_sqlite`]: https://github.com/ivanceras/r2d2-sqlite
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite-session")))]
#[derive(Clone)]
pub struct SqliteSessionStore {
    configuration: CacheConfiguration,
    pool: Pool<SqliteConnectionManager>,
}

#[derive(Clone)]
struct CacheConfiguration {
    cache_keygen: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            cache_keygen: Arc::new(str::to_owned),
        }
    }
}

impl SqliteSessionStore {
    /// Create a new instance of [`SqliteSessionStore`] using the default configuration.
    /// It takes two required inputs to create a new instance of [`SqliteSessionStore`] - a
    /// pool of connections [`Pool<SqliteConnectionManager>`] and a boolean which specifies
    /// whether default garbage connection is enabled/disabled.
    ///
    /// Default garbage collection of stale sessions happens through a database trigger, which
    /// executes before every insert on the `sessions` table.
    pub fn new(
        pool: Pool<SqliteConnectionManager>,
        garbage_collect: bool,
    ) -> Result<Self, anyhow::Error> {
        // create sessions table if it doesn't exist
        let conn = pool.get().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
          id INTEGER NOT NULL,
          session_key	TEXT NOT NULL,
          session_state	TEXT NOT NULL,
          expiry INTEGER NOT NULL,
          PRIMARY KEY(id AUTOINCREMENT)
        )",
            [],
        )
        .map_err(Into::into)
        .map_err(LoadError::Other)?;

        // in order to garbage collect stale sessions, we will use a trigger
        if garbage_collect {
            conn.execute(
                "
            CREATE TRIGGER IF NOT EXISTS garbage_collect 
              BEFORE INSERT 
              ON sessions
            BEGIN
              DELETE FROM sessions WHERE expiry < STRFTIME('%s');
            END;
          ",
                [],
            )
            .map_err(Into::into)
            .map_err(LoadError::Other)?;
        } else {
            conn.execute("DROP TRIGGER IF EXISTS garbage_collect", [])
                .map_err(Into::into)
                .map_err(LoadError::Other)?;
        }

        Ok(Self {
            configuration: CacheConfiguration::default(),
            pool,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl SessionStore for SqliteSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let conn = self.pool.get().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM sessions WHERE session_key = ?")
            .map_err(Into::into)
            .map_err(LoadError::Other)?;

        let value = stmt
            .query(&[&cache_key])
            .map_err(Into::into)
            .map_err(LoadError::Other);

        if let Ok(v) = value {
            // get first result and deserialize `session_state`
            if let Some(session) = v
                .mapped(|row| row.get::<&str, String>("session_state"))
                .nth(0)
            {
                if let Ok(session_state) = session {
                    let deserialized_session_state =
                        serde_json::from_str::<HashMap<String, String>>(session_state.as_str())
                            .map_err(Into::into)
                            .map_err(LoadError::Deserialization)?;

                    return Ok(Some(deserialized_session_state));
                } else {
                    return Ok(None);
                }
            }

            Ok(None)
        } else {
            return Ok(None);
        }
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(SaveError::Serialization)?;

        let session_key = generate_session_key();
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());
        let expiry = OffsetDateTime::now_utc()
            .checked_add(*ttl)
            .unwrap()
            .unix_timestamp();

        let conn = self.pool.get().unwrap();
        let statement = conn
            .prepare("INSERT INTO sessions (session_key, session_state, expiry) VALUES (?, ?, ?)");

        let mut stmt = match statement {
            Ok(v) => v,
            Err(e) => return Err(SaveError::Other(anyhow::anyhow!(e))),
        };

        stmt.insert(params![&cache_key, &body, &expiry,])
            .map_err(Into::into)
            .map_err(SaveError::Other)?;

        Ok(session_key)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::Serialization)?;

        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());
        let expiry = OffsetDateTime::now_utc()
            .checked_add(*ttl)
            .unwrap()
            .unix_timestamp();

        let conn = self.pool.get().unwrap();
        let mut stmt = conn.prepare(
          "UPDATE sessions SET session_state = ?, expiry = ? WHERE session_key = ? AND expiry < STRFTIME('%s')"
        ).map_err(Into::into).map_err(UpdateError::Other)?;

        let v = stmt
            .execute(params![&body, &expiry, &cache_key])
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;
        match v {
            // No rows were updated in the database because the session expired
            // between the load operation and the update operation.
            // Fallback to the `save` routine
            0 => self
                .save(session_state, ttl)
                .await
                .map_err(|err| match err {
                    SaveError::Serialization(err) => UpdateError::Serialization(err),
                    SaveError::Other(err) => UpdateError::Other(err),
                }),
            _val => Ok(session_key),
        }
    }

    async fn update_ttl(&self, session_key: &SessionKey, ttl: &Duration) -> Result<(), Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());
        let expiry = OffsetDateTime::now_utc()
            .checked_add(*ttl)
            .unwrap()
            .unix_timestamp();

        let conn = self.pool.get().unwrap();
        let mut stmt = conn
            .prepare("UPDATE sessions SET expiry = ? WHERE session_key = ?")
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        stmt.execute(params![&expiry, &cache_key])
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let conn = self.pool.get().unwrap();
        let mut stmt = conn
            .prepare("DELETE FROM sessions WHERE session_key = ?")
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        stmt.execute(&[&cache_key])
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use actix_web::cookie::time::{Duration, OffsetDateTime};

    use super::*;
    use crate::test_helpers::acceptance_test_suite;

    fn sqlite_store() -> SqliteSessionStore {
        let manager = SqliteConnectionManager::file("sessions.db");
        let pool = r2d2::Pool::<r2d2_sqlite::SqliteConnectionManager>::new(manager).unwrap();

        SqliteSessionStore::new(pool, true).unwrap()
    }

    #[actix_web::test]
    async fn test_session_workflow() {
        let sqlite_store = sqlite_store();
        acceptance_test_suite(move || sqlite_store.clone(), true).await;
    }

    #[actix_web::test]
    async fn loading_a_missing_session_returns_none() {
        let store = sqlite_store();
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn loading_an_invalid_session_state_returns_deserialization_error() {
        let store = sqlite_store();
        let session_key = generate_session_key();
        store
            .pool
            .get()
            .unwrap()
            .execute(
                "INSERT INTO sessions (session_key, session_state, expiry) VALUES (?, ?, ?)",
                params![
                    session_key.as_ref(),
                    "random-thing-which-is-not-json",
                    OffsetDateTime::now_utc()
                        .checked_add(Duration::hours(1))
                        .unwrap()
                        .unix_timestamp()
                ],
            )
            .unwrap();

        assert!(matches!(
            store.load(&session_key).await.unwrap_err(),
            LoadError::Deserialization(_),
        ));
    }

    #[actix_web::test]
    async fn updating_of_an_expired_state_is_handled_gracefully() {
        let store = sqlite_store();
        let session_key = generate_session_key();
        let initial_session_key = session_key.as_ref().to_owned();
        let updated_session_key = store
            .update(session_key, HashMap::new(), &Duration::seconds(1))
            .await
            .unwrap();
        assert_ne!(initial_session_key, updated_session_key.as_ref());
    }
}
