//! Provider trait definitions.
//!
//! Defines the interface that all authentication providers must implement.

use async_trait::async_trait;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::storage::StorageAdapter;

/// Context passed to providers during initialization
pub struct ProviderContext<S: StorageAdapter> {
    pub storage: std::sync::Arc<S>,
    pub issuer_url: String,
}

/// Subject information returned after successful authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectInfo {
    /// Subject type (e.g., "user", "account")
    pub subject_type: String,
    /// Subject properties (provider-specific)
    pub properties: serde_json::Value,
}

/// Input for client credentials grant
#[derive(Debug)]
pub struct ClientInput {
    pub client_id: String,
    pub client_secret: String,
    pub scope: Option<String>,
}

/// Response from client credentials authentication
#[derive(Debug)]
pub struct ClientResponse {
    pub subject: SubjectInfo,
}

/// Provider trait that all authentication providers must implement.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get the provider name (used in URLs)
    fn name(&self) -> &str;

    /// Get the provider type for UI display
    fn provider_type(&self) -> &str;

    /// Initialize provider routes
    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        ctx: ProviderContext<S>,
    ) -> Router;

    /// Handle client credentials authentication (optional)
    async fn client(&self, _input: ClientInput) -> Option<Result<ClientResponse, String>> {
        None
    }
}

/// OAuth2 provider configuration
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    pub authorization_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub pkce: bool,
}

/// OIDC provider configuration (extends OAuth2)
#[derive(Debug, Clone)]
pub struct OIDCConfig {
    pub oauth2: OAuth2Config,
    pub issuer: String,
    pub jwks_uri: Option<String>,
}
