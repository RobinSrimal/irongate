//! IAM-protected account lifecycle Lambda entry point.

use auth::api::admin::{create_admin_router, AdminAppState};
use auth::config::account_lifecycle::AccountLifecycleConfig;
use auth::DynamoStorage;
use lambda_http::{run, tracing, Error};
use std::sync::{Arc, OnceLock};

static DYNAMO_CLIENT: OnceLock<aws_sdk_dynamodb::Client> = OnceLock::new();

async fn get_dynamo_client() -> &'static aws_sdk_dynamodb::Client {
    if let Some(client) = DYNAMO_CLIENT.get() {
        return client;
    }

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);

    let _ = DYNAMO_CLIENT.set(client);
    DYNAMO_CLIENT.get().expect("dynamo client initialized")
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .json()
        .init();

    tracing::info!("Irongate admin Lambda starting");

    let table_name =
        std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE environment variable required");
    let reuse_mode = std::env::var("AUTH_DELETED_IDENTITY_REUSE")
        .unwrap_or_else(|_| "after_retention".to_string());
    let retention_days = match std::env::var("AUTH_DELETED_IDENTITY_RETENTION_DAYS") {
        Ok(value) => value
            .parse::<u32>()
            .expect("AUTH_DELETED_IDENTITY_RETENTION_DAYS must be a positive integer"),
        Err(_) => 30,
    };
    let lifecycle = AccountLifecycleConfig::from_values(&reuse_mode, retention_days)
        .expect("valid account lifecycle configuration");
    let storage = DynamoStorage::new(get_dynamo_client().await.clone(), table_name);
    let state = AdminAppState {
        storage: Arc::new(storage),
        lifecycle,
    };

    run(create_admin_router(state)).await
}
