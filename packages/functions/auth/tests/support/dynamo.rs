//! Explicit loopback-only SDK clients and disposable DynamoDB Local tables.

use aws_sdk_dynamodb::{
    config::{retry::RetryConfig, timeout::TimeoutConfig, BehaviorVersion, Credentials, Region},
    types::{
        AttributeDefinition, AttributeValue, BillingMode, KeySchemaElement, KeyType,
        ScalarAttributeType,
    },
    Client,
};
use irongate::storage::DynamoStorage;
use irongate::store::keys::StoreKey;
use serde_json::Value;
use std::collections::HashMap;
use std::{future::Future, time::Duration};

pub fn local_endpoint(endpoint: &str) -> Result<url::Url, &'static str> {
    let mut url = url::Url::parse(endpoint).map_err(|_| "invalid local endpoint")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("tests require a plain HTTP loopback endpoint");
    }
    // Pin localhost to a literal address; no DNS/default endpoint fallback is permitted.
    if url.host_str() == Some("localhost") {
        url.set_host(Some("127.0.0.1"))
            .map_err(|_| "invalid local host")?;
    }
    Ok(url)
}

pub fn client(endpoint: &str) -> Client {
    let endpoint = local_endpoint(endpoint).expect("safe local DynamoDB endpoint");
    Client::from_conf(
        aws_sdk_dynamodb::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("eu-west-1"))
            .credentials_provider(Credentials::new("local", "local", None, None, "local-test"))
            .endpoint_url(endpoint.as_str())
            // The Smithy HTTP connector does not follow redirects.
            .http_client(aws_smithy_http_client::Builder::new().build_http())
            .retry_config(RetryConfig::disabled())
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(Duration::from_secs(2))
                    .build(),
            )
            .build(),
    )
}

pub struct LocalTable {
    pub client: Client,
    pub name: String,
    pub storage: DynamoStorage,
}

#[derive(Clone, PartialEq)]
pub struct RawItem {
    pub value: Value,
    pub expiry: Option<i64>,
}

impl RawItem {
    pub fn from_attributes(item: &HashMap<String, AttributeValue>) -> Self {
        Self {
            value: serde_json::from_str(item["value"].as_s().expect("JSON string attribute"))
                .expect("JSON record"),
            expiry: item.get("expiry").map(|value| {
                value
                    .as_n()
                    .expect("numeric TTL")
                    .parse()
                    .expect("Unix seconds")
            }),
        }
    }
}

impl LocalTable {
    pub async fn raw_item(&self, key: &StoreKey) -> Option<RawItem> {
        self.client
            .get_item()
            .table_name(&self.name)
            .consistent_read(true)
            .key("pk", AttributeValue::S(key.pk().into()))
            .key("sk", AttributeValue::S(key.sk().into()))
            .send()
            .await
            .expect("read fixture-owned item")
            .item
            .map(|item| RawItem::from_attributes(&item))
    }
}

pub async fn run<F, Fut>(test: F)
where
    F: FnOnce(LocalTable) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let endpoint = std::env::var("DYNAMODB_LOCAL_ENDPOINT")
        .expect("set DYNAMODB_LOCAL_ENDPOINT to a disposable DynamoDB Local instance");
    let client = client(&endpoint);
    let name = format!("irongate-contract-{}", uuid::Uuid::new_v4().simple());
    let mut create = client
        .create_table()
        .table_name(&name)
        .billing_mode(BillingMode::PayPerRequest);
    for (attribute, key_type) in [("pk", KeyType::Hash), ("sk", KeyType::Range)] {
        create = create
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name(attribute)
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name(attribute)
                    .key_type(key_type)
                    .build()
                    .unwrap(),
            );
    }
    create.send().await.expect("create disposable local table");
    let table = LocalTable {
        storage: DynamoStorage::new(client.clone(), name.clone()),
        client: client.clone(),
        name: name.clone(),
    };
    // Calling the closure inside the task also catches panics before its future is returned.
    let outcome = tokio::spawn(async move { test(table).await }).await;
    client
        .delete_table()
        .table_name(&name)
        .send()
        .await
        .expect("delete only the fixture-owned local table");
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
}
