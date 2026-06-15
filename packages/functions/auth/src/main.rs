#![allow(dead_code, deprecated)]
//! Irongate - Security-first OAuth 2.0 Authorization Server
//!
//! Lambda entry point using Axum for routing.

use lambda_http::{run, tracing, Error};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

mod admin;
mod audit;
mod client;
mod config;
mod core;
mod crypto;
mod error;
mod jwt;
mod oauth;
mod provider;
mod ratelimit;
mod routes;
mod storage;
mod store;
mod subject;
mod ui;

use config::{environment::RuntimeAuthConfig, AppState, Config, ProviderConfig};
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

    tracing::info!("Irongate OAuth 2.0 Lambda starting");

    let dynamo_client = get_dynamo_client().await;
    let table_name =
        std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE environment variable required");
    let storage = DynamoStorage::new(dynamo_client.clone(), table_name);

    let config = Config::from_env();
    let runtime =
        RuntimeAuthConfig::from_env().expect("valid auth runtime configuration required");
    let providers = load_providers_from_env();

    let state = AppState {
        storage: Arc::new(storage),
        config: Arc::new(config),
        runtime: Arc::new(runtime),
        providers: Arc::new(providers),
    };

    let app = create_router(state);
    run(app).await
}

/// Load provider configurations from environment variables.
///
/// Expected env vars per provider:
/// - `PROVIDER_{NAME}_TYPE` = "oauth2" | "oidc" | "password" | "code"
/// - `PROVIDER_{NAME}_CLIENT_ID` (for oauth2/oidc)
/// - `PROVIDER_{NAME}_CLIENT_SECRET` (for oauth2/oidc)
/// - `PROVIDER_{NAME}_AUTH_URL` (for oauth2/oidc)
/// - `PROVIDER_{NAME}_TOKEN_URL` (for oauth2/oidc)
/// - `PROVIDER_{NAME}_SCOPES` (comma-separated, for oauth2/oidc)
/// - `PROVIDER_{NAME}_ISSUER` (for oidc)
/// - `PROVIDER_{NAME}_JWKS_URI` (for oidc, optional)
/// - `PROVIDER_{NAME}_PKCE` = "true" | "false" (for oauth2/oidc, default true)
fn load_providers_from_env() -> HashMap<String, ProviderConfig> {
    let mut providers = HashMap::new();

    // Discover providers from PROVIDERS env var (comma-separated list of names)
    let provider_names = match std::env::var("PROVIDERS") {
        Ok(names) => names,
        Err(_) => return providers,
    };

    for name in provider_names.split(',').map(|s| s.trim()) {
        if name.is_empty() {
            continue;
        }
        let upper = name.to_uppercase();
        let provider_type = std::env::var(format!("PROVIDER_{}_TYPE", upper))
            .unwrap_or_default();

        let config = match provider_type.as_str() {
            "oauth2" => {
                let oauth2_config = load_oauth2_config(&upper);
                oauth2_config.map(ProviderConfig::OAuth2)
            }
            "oidc" => {
                let oauth2_config = load_oauth2_config(&upper);
                oauth2_config.map(|oauth2| {
                    let issuer = std::env::var(format!("PROVIDER_{}_ISSUER", upper))
                        .unwrap_or_default();
                    let jwks_uri = std::env::var(format!("PROVIDER_{}_JWKS_URI", upper)).ok();
                    ProviderConfig::Oidc(provider::traits::OIDCConfig {
                        oauth2,
                        issuer,
                        jwks_uri,
                    })
                })
            }
            "password" => {
                let mut config = provider::password::PasswordConfig::default();
                if std::env::var("DEV_MODE").map(|v| v == "true").unwrap_or(false) {
                    config.require_verification = false;
                }
                Some(ProviderConfig::Password(config))
            }
            "code" => Some(ProviderConfig::Code(
                provider::code::CodeConfig::default(),
            )),
            _ => {
                tracing::warn!("Unknown provider type '{}' for provider '{}'", provider_type, name);
                None
            }
        };

        if let Some(c) = config {
            providers.insert(name.to_string(), c);
        }
    }

    providers
}

fn load_oauth2_config(upper_name: &str) -> Option<provider::traits::OAuth2Config> {
    let client_id = std::env::var(format!("PROVIDER_{}_CLIENT_ID", upper_name)).ok()?;
    let client_secret = std::env::var(format!("PROVIDER_{}_CLIENT_SECRET", upper_name)).ok()?;
    let authorization_url = std::env::var(format!("PROVIDER_{}_AUTH_URL", upper_name)).ok()?;
    let token_url = std::env::var(format!("PROVIDER_{}_TOKEN_URL", upper_name)).ok()?;
    let scopes = std::env::var(format!("PROVIDER_{}_SCOPES", upper_name))
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let pkce = std::env::var(format!("PROVIDER_{}_PKCE", upper_name))
        .map(|v| v != "false")
        .unwrap_or(true);

    Some(provider::traits::OAuth2Config {
        client_id,
        client_secret,
        authorization_url,
        token_url,
        scopes,
        pkce,
    })
}
