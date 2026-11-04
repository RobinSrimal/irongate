//! IAM-protected account lifecycle Lambda entry point.

use auth::api::admin::{create_admin_router, AdminAppState};
use auth::config::account_lifecycle::AccountLifecycleConfig;
use auth::config::audit::AuditLogMode;
use auth::store::AuthStore;
use auth::DynamoStorage;
use lambda_http::{run, tracing, Error};
use std::str::FromStr;
use std::sync::OnceLock;

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
    let audit_log_mode = load_audit_mode(std::env::var("AUTH_AUDIT_LOG_MODE").ok().as_deref());
    let storage = DynamoStorage::new(get_dynamo_client().await.clone(), table_name);
    let state = AdminAppState {
        store: AuthStore::new(storage),
        lifecycle,
        audit: auth::audit::AuditEmitter::production(audit_log_mode),
    };

    run(create_admin_router(state)).await
}

fn load_audit_mode(value: Option<&str>) -> AuditLogMode {
    value
        .map(AuditLogMode::from_str)
        .transpose()
        .expect("valid AUTH_AUDIT_LOG_MODE")
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_audit_mode_is_explicit_or_defaults_to_cloudwatch() {
        assert_eq!(load_audit_mode(None), AuditLogMode::CloudWatch);
        assert_eq!(
            load_audit_mode(Some("cloudwatch")),
            AuditLogMode::CloudWatch
        );
        assert_eq!(load_audit_mode(Some("none")), AuditLogMode::None);
        for invalid in ["", "s3"] {
            assert!(std::panic::catch_unwind(|| load_audit_mode(Some(invalid))).is_err());
        }
    }
}
