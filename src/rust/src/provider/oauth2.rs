//! Base OAuth2 provider implementation.
//!
//! Provides common OAuth2 functionality that other providers can extend.
//! Handles authorization redirect and callback with code exchange.

use super::traits::{OAuth2Config, Provider, ProviderContext};
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;
use serde::{Deserialize, Serialize};

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
        // Provider-specific routes are handled by the generic
        // provider_authorize_handler and provider_callback_handler in routes.rs.
        // The OAuth2Provider is looked up by name at request time.
        router
    }
}

/// Build the OAuth2 authorization URL
pub fn build_authorization_url(
    config: &OAuth2Config,
    state: &str,
    redirect_uri: &str,
    pkce_challenge: Option<&str>,
) -> String {
    let mut url = url::Url::parse(&config.authorization_url).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("state", state)
        .append_pair("scope", &config.scopes.join(" "));

    if let Some(challenge) = pkce_challenge {
        url.query_pairs_mut()
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
    }

    url.to_string()
}

/// Exchange an authorization code for tokens from the external provider.
pub async fn exchange_code(
    config: &OAuth2Config,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, OAuthError> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", &config.client_id),
        ("client_secret", &config.client_secret),
    ];

    let response = client
        .post(&config.token_url)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| OAuthError::ServerError(format!("Token exchange request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        return Err(OAuthError::ServerError(format!(
            "Token exchange failed ({}): {}",
            status, body
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to parse token response: {}", e)))
}

/// Fetch user profile from a userinfo endpoint using the access token.
pub async fn fetch_userinfo(
    userinfo_url: &str,
    access_token: &str,
) -> Result<serde_json::Value, OAuthError> {
    let client = reqwest::Client::new();

    let response = client
        .get(userinfo_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| OAuthError::ServerError(format!("Userinfo request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        return Err(OAuthError::ServerError(format!(
            "Userinfo request failed ({}): {}",
            status, body
        )));
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to parse userinfo response: {}", e)))
}

/// Token response from external OAuth2 provider's token endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    /// OIDC providers return an id_token alongside the access token
    pub id_token: Option<String>,
}

/// State stored during the external OAuth2 flow (maps provider state → session)
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderFlowState {
    /// The irongate authorize session key
    pub session_key: String,
    /// PKCE verifier if used
    pub pkce_verifier: Option<String>,
}

/// Query parameters for the provider authorize redirect
#[derive(Debug, Deserialize)]
pub struct ProviderAuthorizeQuery {
    pub session: String,
}

/// Query parameters for the provider callback
#[derive(Debug, Deserialize)]
pub struct ProviderCallbackQuery {
    pub code: String,
    pub state: String,
}
