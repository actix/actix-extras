use std::{
    ops::Add,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use actix_web::cookie::time::Duration;
use aws_config::{
    default_provider::credentials::DefaultCredentialsChain, meta::region::RegionProviderChain,
    BehaviorVersion,
};
use aws_sdk_dynamodb::{
    config::{Credentials, ProvideCredentials, Region},
    error::SdkError,
    types::AttributeValue,
    Client, Config,
};
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;

use super::SessionKey;
use crate::storage::{
    interface::{LoadError, SaveError, SessionState, UpdateError},
    utils::generate_session_key,
    SessionStore,
};

/// Use DynamoDB as session storage backend.
///
/// ```no_run
/// use actix_web::{web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{SessionMiddleware, storage::DynamoDbSessionStore};
/// use actix_web::cookie::Key;
/// use aws_config::meta::region::RegionProviderChain;
/// use aws_config::Region;
/// use aws_config::default_provider::credentials::DefaultCredentialsChain;
/// use aws_sdk_dynamodb::config::ProvideCredentials;
/// use aws_sdk_dynamodb::Client;
///
///
/// // The secret key would usually be read from a configuration file/environment variables.
/// fn get_secret_key() -> Key {
///     # todo!()
///     // [...]
/// }
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     let secret_key = get_secret_key();
///
///     let dynamo_db_store = DynamoDbSessionStore::builder()
///         .table_name(String::from("MyTableName"))
///         .key_name("PK".to_string())
///         .build()
///         .await?;
///
///     HttpServer::new(move ||
///             App::new()
///             .wrap(SessionMiddleware::new(
///                 dynamo_db_store.clone(),
///                 secret_key.clone()
///             ))
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// # Implementation notes
/// `DynamoDbSessionStore` leverages [`aws-sdk-dynamodb`] as a DynamoDB client.
///
/// [`aws-sdk-dynamodb`]: https://github.com/awslabs/aws-sdk-rust
#[derive(Clone)]
pub struct DynamoDbSessionStore {
    configuration: CacheConfiguration,
    client: Client,
}


/// Struct for configuring the DynamoDB Session Store. To add custom configuration, use the `DynamoDbSessionStoreBuilder`
///
/// [`DynamoDbSessionStoreBuilder`]: crate::storage::DynamoDbSessionStoreBuilder
#[derive(Clone)]
pub struct CacheConfiguration {
    cache_keygen: Arc<dyn Fn(&str) -> String + Send + Sync>,
    table_name: String,
    key_name: String,
    ttl_name: String,
    session_data_name: String,
    use_dynamo_db_local: bool,
    dynamo_db_local_endpoint: String,
    sdk_config: Option<Config>,
    region: Option<Region>,
    credentials: Option<Credentials>,
}

impl Default for CacheConfiguration {
    /// Default values for the cache configuration
    ///
    /// ```no_run
    /// Self {
    ///     cache_keygen: Arc::new(str::to_owned),
    ///     table_name: "sessions".to_string(),
    ///     use_dynamo_db_local: false,
    ///     key_name: "SessionId".to_string(),
    ///     ttl_name: "session_ttl".to_string(),
    ///     session_data_name: "session_data".to_string(),
    ///     dynamo_db_local_endpoint: "http://localhost:8000".to_string(),
    ///     sdk_config: None,
    ///     region: None,
    ///     credentials: None,
    /// }
    /// ```
    fn default() -> Self {
        Self {
            cache_keygen: Arc::new(str::to_owned),
            table_name: "sessions".to_string(),
            use_dynamo_db_local: false,
            key_name: "SessionId".to_string(),
            ttl_name: "session_ttl".to_string(),
            session_data_name: "session_data".to_string(),
            dynamo_db_local_endpoint: "http://localhost:8000".to_string(),
            sdk_config: None,
            region: None,
            credentials: None,
        }
    }
}

impl DynamoDbSessionStore {
    /// A fluent API to configure [`DynamoDbSessionStore`].
    /// It takes as input the only required input to create a new instance of [`DynamoDbSessionStore`].
    /// As a default, it expects a DynamoDB table name of 'sessions', with a single partition key of 'SessionId' this can be overridden using the [`DynamoDbSessionStoreBuilder`].
    pub fn builder() -> DynamoDbSessionStoreBuilder {
        DynamoDbSessionStoreBuilder {
            configuration: CacheConfiguration::default(),
        }
    }

    /// Create a new instance of [`DynamoDbSessionStore`] using the default configuration..
    pub async fn new() -> Result<DynamoDbSessionStore, anyhow::Error> {
        Self::builder().build().await
    }
}

/// A fluent builder to construct a [`DynamoDbSessionStore`] instance with custom configuration
/// parameters.
///
/// [`DynamoDbSessionStore`]: crate::storage::DynamoDbSessionStore
#[must_use]
pub struct DynamoDbSessionStoreBuilder {
    configuration: CacheConfiguration,
}

impl DynamoDbSessionStoreBuilder {
    /// Set a custom cache key generation strategy, expecting a session key as input.
    pub fn cache_keygen<F>(mut self, keygen: F) -> Self
    where
        F: Fn(&str) -> String + 'static + Send + Sync,
    {
        self.configuration.cache_keygen = Arc::new(keygen);
        self
    }
    /// Set the DynamoDB table name to use.
    pub fn table_name(mut self, table_name: String) -> Self {
        self.configuration.table_name = table_name;
        self
    }
    /// Set if DynamoDB local should be used, useful for local testing.
    pub fn use_dynamo_db_local(mut self, should_use: bool) -> Self {
        self.configuration.use_dynamo_db_local = should_use;
        self.configuration.dynamo_db_local_endpoint = "http://localhost:8000".to_string();
        self
    }

    /// Set the credentials to use for the DynamoDB client.
    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.configuration.credentials = Some(credentials);

        self
    }

    /// Set the SDK config to use for the DynamoDB client.
    pub fn with_sdk_config(mut self, config: Config) -> Self {
        self.configuration.sdk_config = Some(config);

        self
    }

    /// Set the region to use for the DynamoDB client.
    pub fn with_region(mut self, region: Region) -> Self {
        self.configuration.region = Some(region);

        self
    }

    /// Set the endpoint to use if using DynamoDB Local. Defaults to 'http://localhost:8000'.
    pub fn dynamo_db_local_endpoint(mut self, dynamo_db_local_endpoint: String) -> Self {
        self.configuration.dynamo_db_local_endpoint = dynamo_db_local_endpoint;
        self
    }

    /// Set the name of the DynamoDB partition key.
    pub fn key_name(mut self, key_name: String) -> Self {
        self.configuration.key_name = key_name;
        self
    }

    /// Set the name of the DynamoDB column to use for the ttl. Defaults to 'ttl'.
    pub fn ttl_name(mut self, ttl_name: String) -> Self {
        self.configuration.ttl_name = ttl_name;
        self
    }

    /// Set the name of the DynamoDB column to use for the session data. Defaults to 'session_data'.
    pub fn session_data_name(mut self, session_data_name: String) -> Self {
        self.configuration.session_data_name = session_data_name;
        self
    }

    /// Finalise the builder and return a [`DynamoDbSessionStore`] instance.
    ///
    /// [`DynamoDbSessionStore`]: crate::storage::DynamoDbSessionStore
    pub async fn build(self) -> Result<DynamoDbSessionStore, anyhow::Error> {
        let region = match &self.configuration.region {
            None => make_region_provider().region().await.unwrap(),
            Some(r) => r.clone(),
        };

        let credentials = match &self.configuration.credentials {
            None => make_credentials_provider(region.clone()).await?,
            Some(c) => c.clone(),
        };

        let conf = match &self.configuration.sdk_config {
            None => make_config(&region, &credentials, &self.configuration).await?,
            Some(config) => config.clone(),
        };

        let dynamodb_client = aws_sdk_dynamodb::Client::from_conf(conf.clone());
        Ok(DynamoDbSessionStore {
            configuration: self.configuration,
            client: dynamodb_client,
        })
    }
}

impl SessionStore for DynamoDbSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<SessionState>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let cache_value = self
            .client
            .get_item()
            .table_name(&self.configuration.table_name)
            .key(&self.configuration.key_name, AttributeValue::S(cache_key))
            .send()
            .await
            .map_err(Into::into)
            .map_err(LoadError::Other)?;

        match cache_value.item {
            None => Ok(None),
            Some(item) => match item["session_data"].as_s() {
                Ok(value) => Ok(serde_json::from_str(value)
                    .map_err(Into::into)
                    .map_err(LoadError::Deserialization)?),
                Err(_) => Ok(None),
            },
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

        let _ = self
            .client
            .put_item()
            .table_name(&self.configuration.table_name)
            .item(&self.configuration.key_name, AttributeValue::S(cache_key))
            .item("session_data", AttributeValue::S(body))
            .item(
                &self.configuration.ttl_name,
                AttributeValue::N(get_epoch_ms(*ttl).to_string()),
            )
            .condition_expression(
                format!("attribute_not_exists({})", self.configuration.key_name).to_string(),
            )
            .send()
            .await
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

        let put_res = self
            .client
            .put_item()
            .table_name(&self.configuration.table_name)
            .item(&self.configuration.key_name, AttributeValue::S(cache_key))
            .item("session_data", AttributeValue::S(body))
            .item(
                &self.configuration.ttl_name,
                AttributeValue::N(get_epoch_ms(*ttl).to_string()),
            )
            .condition_expression(
                format!("attribute_exists({})", self.configuration.key_name).to_string(),
            )
            .send()
            .await;

        match put_res {
            Ok(_) => Ok(session_key),
            Err(err) => {
                match err {
                    // A response error can occur if the condition expression checking the session exists fails
                    // // This can happen if the session state expired between the load operation and the
                    // update operation. Unlucky, to say the least. We fall back to the `save` routine
                    // to ensure that the new key is unique.
                    SdkError::ResponseError(_resp_err) => self
                        .save(session_state, ttl)
                        .await
                        .map_err(|err| match err {
                            SaveError::Serialization(err) => UpdateError::Serialization(err),
                            SaveError::Other(err) => UpdateError::Other(err),
                        }),
                    SdkError::ServiceError(_resp_err) => self
                        .save(session_state, ttl)
                        .await
                        .map_err(|err| match err {
                            SaveError::Serialization(err) => UpdateError::Serialization(err),
                            SaveError::Other(err) => UpdateError::Other(err),
                        }),
                    _ => Err(UpdateError::Other(anyhow::anyhow!(
                        "Failed to update session state. {:?}",
                        err
                    ))),
                }
            }
        }
    }

    async fn update_ttl(
        &self,
        session_key: &SessionKey,
        ttl: &Duration,
    ) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let _update_res = self
            .client
            .update_item()
            .table_name(&self.configuration.table_name)
            .key(&self.configuration.key_name, AttributeValue::S(cache_key))
            .update_expression("SET ttl = :value")
            .expression_attribute_values(
                ":value",
                AttributeValue::N(get_epoch_ms(*ttl).to_string()),
            )
            .send()
            .await;

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.client
            .delete_item()
            .table_name(&self.configuration.table_name)
            .key(&self.configuration.key_name, AttributeValue::S(cache_key))
            .send()
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(())
    }
}

fn get_epoch_ms(duration: Duration) -> u128 {
    SystemTime::now()
        .add(duration)
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

async fn make_credentials_provider(region: Region) -> Result<Credentials, anyhow::Error> {
    Ok(DefaultCredentialsChain::builder()
        .region(region.clone())
        .build()
        .await
        .provide_credentials()
        .await
        .unwrap())
}

async fn make_config(
    region: &Region,
    credentials: &Credentials,
    configuration: &CacheConfiguration,
) -> Result<Config, anyhow::Error> {
    let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();

    let hyper_client = HyperClientBuilder::new().build(https_connector);

    let conf_builder = Config::builder()
        .behavior_version(BehaviorVersion::v2023_11_09())
        .credentials_provider(credentials.clone())
        .http_client(hyper_client)
        .region(region.clone());

    Ok(match configuration.use_dynamo_db_local {
        true => conf_builder
            .endpoint_url(configuration.dynamo_db_local_endpoint.clone())
            .build(),
        false => conf_builder.build(),
    })
}

fn make_region_provider() -> RegionProviderChain {
    RegionProviderChain::default_provider().or_else(Region::new("us-east-1"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use actix_web::cookie::time;

    use super::*;
    use crate::test_helpers::acceptance_test_suite;

    async fn dynamo_store() -> DynamoDbSessionStore {
        DynamoDbSessionStore::builder()
            .table_name("auth".to_string())
            .key_name("PK".to_string())
            .ttl_name("session_ttl".to_string())
            .use_dynamo_db_local(true)
            .dynamo_db_local_endpoint("http://localhost:8000".to_string())
            .build()
            .await
            .unwrap()
    }

    #[actix_web::test]
    async fn test_session_workflow() {
        let dynamo_store = dynamo_store().await;
        acceptance_test_suite(move || dynamo_store.clone(), true).await;
    }

    #[actix_web::test]
    async fn loading_a_missing_session_returns_none() {
        let store = dynamo_store().await;
        let session_key = generate_session_key();
        assert!(store.load(&session_key).await.unwrap().is_none());
    }

    #[actix_web::test]
    async fn loading_an_invalid_session_state_returns_deserialization_error() {
        let store = dynamo_store().await;
        let session_key = generate_session_key();
        store
            .client
            .clone()
            .put_item()
            .table_name(&store.configuration.table_name)
            .item(
                &store.configuration.key_name,
                AttributeValue::S(session_key.as_ref().to_string()),
            )
            .item(
                "session_data",
                AttributeValue::S("random-thing-that-is-not-json".to_string()),
            )
            .item(
                &store.configuration.ttl_name,
                AttributeValue::N(get_epoch_ms(Duration::seconds(10)).to_string()),
            )
            .send()
            .await
            .unwrap();

        assert!(matches!(
            store.load(&session_key).await.unwrap_err(),
            LoadError::Deserialization(_),
        ));
    }

    #[actix_web::test]
    async fn updating_of_an_expired_state_is_handled_gracefully() {
        let store = dynamo_store().await;
        let session_key = generate_session_key();
        let initial_session_key = session_key.as_ref().to_owned();
        let updated_session_key = store
            .update(session_key, HashMap::new(), &time::Duration::seconds(1))
            .await
            .unwrap();
        assert_ne!(initial_session_key, updated_session_key.as_ref());
    }
}
