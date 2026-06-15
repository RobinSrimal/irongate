#![allow(dead_code, deprecated)]
//! Irongate - Security-first OAuth 2.0 Authorization Server
//!
//! Lambda entry point using Axum for routing.

use lambda_http::{run, tracing, Error};
use std::sync::{Arc, OnceLock};

mod api;
mod audit;
mod client;
mod config;
mod core;
mod crypto;
mod email;
mod error;
mod oauth;
mod providers;
mod ratelimit;
mod routes;
mod storage;
mod store;
mod subject;

use config::{environment::RuntimeAuthConfig, AppState, Config};
use email::ResendEmailSender;
use routes::create_router;
use storage::DynamoStorage;
use store::AuthStore;

/// Global DynamoDB client (initialized once per Lambda instance)
static DYNAMO_CLIENT: OnceLock<aws_sdk_dynamodb::Client> = OnceLock::new();
static AWS_CONFIG: OnceLock<aws_config::SdkConfig> = OnceLock::new();

pub async fn get_aws_config() -> &'static aws_config::SdkConfig {
    if let Some(config) = AWS_CONFIG.get() {
        return config;
    }

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let _ = AWS_CONFIG.set(config);
    AWS_CONFIG.get().unwrap()
}

/// Get or initialize the DynamoDB client
pub async fn get_dynamo_client() -> &'static aws_sdk_dynamodb::Client {
    if let Some(client) = DYNAMO_CLIENT.get() {
        return client;
    }

    let config = get_aws_config().await;
    let client = aws_sdk_dynamodb::Client::new(config);

    // Try to set it; if another task beat us, use theirs
    let _ = DYNAMO_CLIENT.set(client);
    DYNAMO_CLIENT.get().unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .json()
        .init();

    tracing::info!("Irongate OAuth 2.0 Lambda starting");

    let aws_config = get_aws_config().await;
    let dynamo_client = get_dynamo_client().await;
    let table_name =
        std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE environment variable required");
    let storage = DynamoStorage::new(dynamo_client.clone(), table_name);

    let config = Config::from_env();
    let runtime = RuntimeAuthConfig::from_env_with_aws_config(aws_config)
        .await
        .expect("valid auth runtime configuration required");
    let email_sender = ResendEmailSender::new(runtime.email.clone());

    let state = AppState {
        store: AuthStore::new(storage),
        config: Arc::new(config),
        runtime: Arc::new(runtime),
        email_sender: Arc::new(email_sender),
        google_client: Arc::new(crate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(crate::providers::apple::ReqwestAppleOidcClient::new()),
    };

    let app = create_router(state);
    run(app).await
}
