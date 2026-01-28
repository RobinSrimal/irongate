//! Token endpoint (/token).
//!
//! Handles token exchange for all grant types.

use axum::{
    extract::State,
    http::HeaderMap,
    response::Json,
    Form,
};
use serde::{Deserialize, Serialize};

use crate::client::validate_token_request;
use crate::config::AppState;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;

/// Token request form data
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    /// Grant type (required)
    pub grant_type: String,
    /// Client ID (required for public clients)
    pub client_id: Option<String>,
    /// Client secret (required for confidential clients with client_secret_post)
    pub client_secret: Option<String>,
    /// Authorization code (for authorization_code grant)
    pub code: Option<String>,
    /// Redirect URI (for authorization_code grant)
    pub redirect_uri: Option<String>,
    /// PKCE code verifier (for authorization_code grant)
    pub code_verifier: Option<String>,
    /// Refresh token (for refresh_token grant)
    pub refresh_token: Option<String>,
    /// Scope (optional)
    pub scope: Option<String>,
}

/// Token response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Handle the token request.
pub async fn handle_token<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Form(params): Form<TokenRequest>,
) -> Result<Json<TokenResponse>, OAuthError> {
    // Extract client_id from request or Basic auth
    let client_id = params
        .client_id
        .as_deref()
        .or_else(|| extract_client_id_from_basic_auth(&headers))
        .ok_or_else(|| OAuthError::InvalidRequest("client_id required".to_string()))?;

    // Get Authorization header for client_secret_basic
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    // Validate client
    let client = validate_token_request(
        state.storage.as_ref(),
        client_id,
        params.client_secret.as_deref(),
        &params.grant_type,
        auth_header,
    )
    .await?;

    // Handle based on grant type
    match params.grant_type.as_str() {
        "authorization_code" => {
            handle_authorization_code_grant(&state, &params, &client).await
        }
        "refresh_token" => {
            handle_refresh_token_grant(&state, &params, &client).await
        }
        "client_credentials" => {
            handle_client_credentials_grant(&state, &params, &client).await
        }
        _ => Err(OAuthError::UnsupportedGrantType(params.grant_type)),
    }
}

fn extract_client_id_from_basic_auth(headers: &HeaderMap) -> Option<&str> {
    // TODO: Parse Basic auth header
    None
}

async fn handle_authorization_code_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &crate::client::Client,
) -> Result<Json<TokenResponse>, OAuthError> {
    let code = params
        .code
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("code required".to_string()))?;

    let redirect_uri = params
        .redirect_uri
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("redirect_uri required".to_string()))?;

    // CRITICAL: Validate PKCE if required
    if client.pkce_required {
        let verifier = params
            .code_verifier
            .as_ref()
            .ok_or_else(|| OAuthError::InvalidRequest("code_verifier required".to_string()))?;

        // TODO: Validate PKCE with constant-time comparison
    }

    // TODO: Validate code and exchange for tokens
    // CRITICAL: Delete code BEFORE generating tokens to prevent race conditions

    todo!("Implement authorization code exchange")
}

async fn handle_refresh_token_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &crate::client::Client,
) -> Result<Json<TokenResponse>, OAuthError> {
    let refresh_token = params
        .refresh_token
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("refresh_token required".to_string()))?;

    // TODO: Implement atomic refresh token rotation using DynamoDB transactions

    todo!("Implement refresh token rotation")
}

async fn handle_client_credentials_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &crate::client::Client,
) -> Result<Json<TokenResponse>, OAuthError> {
    // Client credentials already validated in validate_token_request

    // TODO: Generate access token for the client

    todo!("Implement client credentials grant")
}
