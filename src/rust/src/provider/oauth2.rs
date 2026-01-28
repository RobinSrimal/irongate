//! Base OAuth2 provider implementation.
//!
//! Provides common OAuth2 functionality that other providers can extend.

use super::traits::{OAuth2Config, Provider, ProviderContext, SubjectInfo};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

/// Base OAuth2 provider
pub struct OAuth2Provider {
    pub name: String,
    pub config: OAuth2Config,
}

impl OAuth2Provider {
    pub fn new(name: impl Into<String>, config: OAuth2Config) -> Self {
        Self {
            name: name.into(),
            config,
        }
    }
}

#[async_trait]
impl Provider for OAuth2Provider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &str {
        "oauth2"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        _ctx: ProviderContext<S>,
    ) -> Router {
        // TODO: Add provider-specific routes
        router
    }
}

/// Build the OAuth2 authorization URL
pub fn build_authorization_url(config: &OAuth2Config, state: &str, redirect_uri: &str) -> String {
    let mut url = url::Url::parse(&config.authorization_url).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("state", state)
        .append_pair("scope", &config.scopes.join(" "));

    url.to_string()
}

/// Exchange an authorization code for tokens
pub async fn exchange_code(
    config: &OAuth2Config,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    todo!("Implement OAuth2 code exchange")
}

/// Token response from OAuth2 token endpoint
#[derive(Debug, serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}
