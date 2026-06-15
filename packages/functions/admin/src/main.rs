//! IAM-protected account lifecycle Lambda entry point.

use auth::api::admin::{create_admin_router, AdminAppState};
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
    let storage = DynamoStorage::new(get_dynamo_client().await.clone(), table_name);
    let state = AdminAppState {
        storage: Arc::new(storage),
    };

    run(create_admin_router(state)).await
}
