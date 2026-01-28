//! Irongate - Security-first OAuth 2.0 Authorization Server
//!
//! Lambda entry point using Axum for routing.

use lambda_http::{run, tracing, Error};
use std::sync::OnceLock;

mod admin;
mod client;
mod config;
mod crypto;
mod error;
mod jwt;
mod oauth;
mod provider;
mod ratelimit;
mod routes;
mod storage;
mod subject;
mod ui;

use routes::create_router;
use storage::DynamoStorage;

/// Global DynamoDB client (initialized once per Lambda instance)
static DYNAMO_CLIENT: OnceLock<aws_sdk_dynamodb::Client> = OnceLock::new();

/// Get or initialize the DynamoDB client
pub async fn get_dynamo_client() -> &'static aws_sdk_dynamodb::Client {
    if let Some(client) = DYNAMO_CLIENT.get() {
        return client;
    }

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);

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

    tracing::info!("Irongate OAuth 2.0 server starting");

    // Initialize storage
    let dynamo_client = get_dynamo_client().await;
    let table_name =
        std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE environment variable required");
    let storage = DynamoStorage::new(dynamo_client.clone(), table_name);

    // Create Axum router
    let app = create_router(storage);

    // Run the Lambda handler
    run(app).await
}
