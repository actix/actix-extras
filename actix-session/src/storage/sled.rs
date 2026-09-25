use std::path::Path;

use actix_web::{
    cookie::time::{Duration, OffsetDateTime},
    web,
};

use super::{
    format::{deserialize_session_state, serialize_session_state},
    interface::SessionState,
    utils::generate_session_key,
    LoadError, SaveError, SessionKey, SessionStore, UpdateError,
};

/// Store session state in a local sled database.
///
/// Enable the `sled-session` feature to use this backend. Open one store before creating the HTTP
/// server, then clone it for each worker. The database directory must be dedicated to sessions and
/// can only be opened by one process at a time. This backend is suitable for a single server; use a
/// shared backend such as Redis when several processes need access to the same sessions.
///
/// Expiry times are stored with the session state and remain valid after a restart. Expired sessions
/// are removed when accessed. Call [`Self::purge_expired`] periodically to remove sessions that are
/// no longer accessed. No background cleanup task is started by this store.
///
/// Database operations run on Actix Web's blocking thread pool. Sled flushes writes periodically;
/// a process crash can lose recent updates or invalidations that have not yet reached disk. Call
/// [`Self::flush`] when the application requires those writes to be durable.
///
/// ```no_run
/// use actix_session::{storage::SledSessionStore, SessionMiddleware};
/// use actix_web::{cookie::Key, App, HttpServer};
///
/// # #[actix_web::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let store = SledSessionStore::new("./sessions.db").await?;
/// // Load a stable secret key from configuration in production.
/// let key = Key::generate();
///
/// HttpServer::new(move || {
///     App::new().wrap(SessionMiddleware::new(store.clone(), key.clone()))
/// })
/// .bind(("127.0.0.1", 8080))?
/// .run()
/// .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SledSessionStore {
    db: sled::Db,
}

impl SledSessionStore {
    /// Open or create a session database at `db_path`.
    ///
    /// Returns an error if the database cannot be opened, for example if another process holds its
    /// lock. Clone the returned store to share it between workers instead of opening the path again.
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let db_path = db_path.as_ref().to_owned();
        let db = web::block(move || sled::open(db_path)).await??;

        Ok(Self { db })
    }

    /// Remove expired sessions, including sessions that are no longer accessed.
    ///
    /// Returns the number of removed records. This scans the database on a blocking thread. A
    /// concurrent session update is preserved. An unreadable record or storage error stops the scan
    /// and returns an error; records already removed remain removed.
    pub async fn purge_expired(&self) -> Result<usize, anyhow::Error> {
        let db = self.db.clone();

        web::block(move || {
            let now = OffsetDateTime::now_utc().unix_timestamp_nanos();
            let mut removed = 0;

            for entry in db.iter() {
                let (key, value) = entry?;

                if expires_at(&value)? <= now
                    && db
                        .compare_and_swap(key, Some(value), None::<&[u8]>)?
                        .is_ok()
                {
                    removed += 1;
                }
            }

            Ok(removed)
        })
        .await?
    }

    /// Flush pending session writes and invalidations to disk.
    pub async fn flush(&self) -> Result<(), anyhow::Error> {
        self.db.flush_async().await?;

        Ok(())
    }
}

impl SessionStore for SledSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let db = self.db.clone();
        let key = session_key.as_ref().to_owned();

        web::block(move || loop {
            let Some(value) = db.get(&key).map_err(|err| LoadError::Other(err.into()))? else {
                return Ok(None);
            };

            let expiry = expires_at(&value).map_err(LoadError::Deserialization)?;

            if expiry <= OffsetDateTime::now_utc().unix_timestamp_nanos() {
                // Retry if another request refreshed the session after the read.
                if db
                    .compare_and_swap(&key, Some(value), None::<&[u8]>)
                    .map_err(|err| LoadError::Other(err.into()))?
                    .is_err()
                {
                    continue;
                }

                return Ok(None);
            }

            let state = std::str::from_utf8(&value[EXPIRY_BYTES..])
                .map_err(|err| LoadError::Deserialization(err.into()))?;

            return deserialize_session_state(state)
                .map(Some)
                .map_err(LoadError::Deserialization);
        })
        .await
        .map_err(|err| LoadError::Other(err.into()))?
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let body = encode_session(&session_state, ttl).map_err(SaveError::Serialization)?;
        let db = self.db.clone();

        web::block(move || insert_session(&db, &body))
            .await
            .map_err(|err| SaveError::Other(err.into()))?
            .map_err(SaveError::Other)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: SessionState,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let body = encode_session(&session_state, ttl).map_err(UpdateError::Serialization)?;
        let db = self.db.clone();

        web::block(move || -> Result<_, anyhow::Error> {
            loop {
                let Some(value) = db.get(session_key.as_ref())? else {
                    return insert_session(&db, &body);
                };

                if expires_at(&value)? <= OffsetDateTime::now_utc().unix_timestamp_nanos() {
                    if db
                        .compare_and_swap(session_key.as_ref(), Some(value), None::<&[u8]>)?
                        .is_err()
                    {
                        continue;
                    }

                    return insert_session(&db, &body);
                }

                if db
                    .compare_and_swap(session_key.as_ref(), Some(value), Some(body.as_slice()))?
                    .is_ok()
                {
                    return Ok(session_key);
                }
            }
        })
        .await
        .map_err(|err| UpdateError::Other(err.into()))?
        .map_err(UpdateError::Other)
    }

    async fn update_ttl(
        &self,
        session_key: &SessionKey,
        ttl: &Duration,
    ) -> Result<(), anyhow::Error> {
        let db = self.db.clone();
        let key = session_key.as_ref().to_owned();
        let expiry = deadline(ttl).to_be_bytes();

        web::block(move || {
            loop {
                let Some(value) = db.get(&key)? else {
                    return Ok(());
                };

                let replacement =
                    if expires_at(&value)? <= OffsetDateTime::now_utc().unix_timestamp_nanos() {
                        None
                    } else {
                        let mut refreshed = value.to_vec();
                        refreshed[..EXPIRY_BYTES].copy_from_slice(&expiry);

                        Some(refreshed)
                    };

                // Compare the whole record so a refresh cannot overwrite a concurrent state update
                // or restore a session that another request deleted.
                if db.compare_and_swap(&key, Some(value), replacement)?.is_ok() {
                    return Ok(());
                }
            }
        })
        .await?
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let db = self.db.clone();
        let key = session_key.as_ref().to_owned();

        web::block(move || db.remove(key)).await??;

        Ok(())
    }
}

// A signed Unix timestamp in nanoseconds precedes the shared, versioned JSON session format. The
// fixed-size prefix lets TTL refreshes and cleanup inspect expiry without decoding session data.
const EXPIRY_BYTES: usize = size_of::<i128>();

fn deadline(ttl: &Duration) -> i128 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() + ttl.whole_nanoseconds()
}

fn expires_at(value: &[u8]) -> Result<i128, anyhow::Error> {
    let expiry = value
        .get(..EXPIRY_BYTES)
        .ok_or_else(|| anyhow::anyhow!("Session expiry is missing"))?;

    Ok(i128::from_be_bytes(expiry.try_into()?))
}

fn encode_session(state: &SessionState, ttl: &Duration) -> Result<Vec<u8>, anyhow::Error> {
    let state = serialize_session_state(state)?;
    let mut value = Vec::with_capacity(EXPIRY_BYTES + state.len());
    value.extend_from_slice(&deadline(ttl).to_be_bytes());
    value.extend_from_slice(state.as_bytes());

    Ok(value)
}

fn insert_session(db: &sled::Db, body: &[u8]) -> Result<SessionKey, anyhow::Error> {
    loop {
        let key = generate_session_key();

        if db
            .compare_and_swap(key.as_ref(), None::<&[u8]>, Some(body))?
            .is_ok()
        {
            return Ok(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::acceptance_test_suite;

    fn sled_db() -> SledSessionStore {
        SledSessionStore {
            db: sled::Config::new().temporary(true).open().unwrap(),
        }
    }

    #[actix_web::test]
    async fn session_workflow() {
        let store = sled_db();
        acceptance_test_suite(move || store.clone(), true).await;
    }

    #[actix_web::test]
    async fn loading_a_missing_session_returns_none() {
        let store = sled_db();
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn deleting_a_session_invalidates_its_key() {
        let store = sled_db();
        let key = store
            .save(SessionState::new(), &Duration::minutes(5))
            .await
            .unwrap();

        store.delete(&key).await.unwrap();

        assert!(store.load(&key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn loading_an_expired_session_returns_none() {
        let store = sled_db();
        let key = store
            .save(SessionState::new(), &Duration::seconds(-1))
            .await
            .unwrap();

        assert!(store.load(&key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn loading_an_invalid_session_state_returns_deserialization_error() {
        let store = sled_db();
        let session_key = generate_session_key();

        store
            .db
            .insert(session_key.as_ref(), "random-thing-which-is-not-json")
            .unwrap();

        assert!(matches!(
            store.load(&session_key).await.unwrap_err(),
            LoadError::Deserialization(_),
        ));
    }

    #[actix_web::test]
    async fn updating_an_expired_session_generates_a_new_key() {
        let store = sled_db();
        let session_key = store
            .save(SessionState::new(), &Duration::seconds(-1))
            .await
            .unwrap();
        let initial_session_key = session_key.as_ref().to_owned();

        let updated_session_key = store
            .update(session_key, SessionState::new(), &Duration::minutes(5))
            .await
            .unwrap();

        assert_ne!(initial_session_key, updated_session_key.as_ref());
        assert!(store.load(&updated_session_key).await.unwrap().is_some());
    }

    #[actix_web::test]
    async fn updating_a_missing_session_generates_a_new_key() {
        let store = sled_db();
        let key = generate_session_key();
        let old_key = key.as_ref().to_owned();
        let state: SessionState =
            serde_json::from_value(serde_json::json!({"counter": 42})).unwrap();

        let key = store
            .update(key, state, &Duration::minutes(5))
            .await
            .unwrap();

        assert_ne!(key.as_ref(), old_key);
        assert_eq!(store.load(&key).await.unwrap().unwrap()["counter"], 42);
    }

    #[actix_web::test]
    async fn updating_a_live_session_preserves_its_key_and_changes_state_and_ttl() {
        let store = sled_db();
        let key = store
            .save(SessionState::new(), &Duration::minutes(5))
            .await
            .unwrap();
        let old_key = key.as_ref().to_owned();
        let state: SessionState =
            serde_json::from_value(serde_json::json!({"counter": 42})).unwrap();

        let key = store
            .update(key, state.clone(), &Duration::minutes(5))
            .await
            .unwrap();

        assert_eq!(key.as_ref(), old_key);
        assert_eq!(store.load(&key).await.unwrap(), Some(state.clone()));

        let key = store
            .update(key, state, &Duration::seconds(-1))
            .await
            .unwrap();

        assert!(store.load(&key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn refreshing_ttl_extends_a_session_without_changing_its_state() {
        let store = sled_db();
        let state: SessionState = serde_json::from_value(serde_json::json!({
            "text": "hello", "number": 42, "nested": {"enabled": true}
        }))
        .unwrap();
        let key = store
            .save(state.clone(), &Duration::seconds(2))
            .await
            .unwrap();

        store.update_ttl(&key, &Duration::minutes(5)).await.unwrap();
        actix_web::rt::time::sleep(std::time::Duration::from_millis(2100)).await;

        assert_eq!(store.load(&key).await.unwrap(), Some(state));

        store
            .update_ttl(&key, &Duration::seconds(-1))
            .await
            .unwrap();

        assert!(store.load(&key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn refreshing_ttl_does_not_restore_expired_or_deleted_sessions() {
        let store = sled_db();
        let expired = store
            .save(SessionState::new(), &Duration::ZERO)
            .await
            .unwrap();
        let deleted = store
            .save(SessionState::new(), &Duration::minutes(5))
            .await
            .unwrap();
        store.delete(&deleted).await.unwrap();

        for key in [&expired, &deleted] {
            store.update_ttl(key, &Duration::minutes(5)).await.unwrap();

            assert!(store.load(key).await.unwrap().is_none());
        }
    }

    #[actix_web::test]
    async fn cleanup_removes_only_expired_sessions() {
        let store = sled_db();
        let expired = store
            .save(SessionState::new(), &Duration::ZERO)
            .await
            .unwrap();
        let live = store
            .save(SessionState::new(), &Duration::minutes(5))
            .await
            .unwrap();

        assert_eq!(store.purge_expired().await.unwrap(), 1);
        assert_eq!(store.purge_expired().await.unwrap(), 0);
        assert!(store.load(&expired).await.unwrap().is_none());
        assert!(store.load(&live).await.unwrap().is_some());
    }

    #[actix_web::test]
    async fn sessions_and_expiry_survive_reopening_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let store = SledSessionStore::new(dir.path()).await.unwrap();
        let state: SessionState =
            serde_json::from_value(serde_json::json!({"counter": 42})).unwrap();
        let live = store
            .save(state.clone(), &Duration::minutes(5))
            .await
            .unwrap();
        let expired = store
            .save(SessionState::new(), &Duration::ZERO)
            .await
            .unwrap();
        let deleted = store
            .save(SessionState::new(), &Duration::minutes(5))
            .await
            .unwrap();
        store.delete(&deleted).await.unwrap();
        store.flush().await.unwrap();
        drop(store);

        let store = SledSessionStore::new(dir.path()).await.unwrap();

        assert_eq!(store.load(&live).await.unwrap(), Some(state));
        assert!(store.load(&expired).await.unwrap().is_none());
        assert!(store.load(&deleted).await.unwrap().is_none());
    }
}
