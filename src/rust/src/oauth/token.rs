//! Token endpoint (/token).
//!
//! Handles token exchange for all grant types:
//! - authorization_code: Exchange auth code for tokens (with PKCE)
//! - refresh_token: Atomic rotation via DynamoDB transactions
//! - client_credentials: Direct token issuance for confidential clients

use axum::{
    extract::State,
    http::HeaderMap,
    response::Json,
    Form,
};
use serde::{Deserialize, Serialize};

use crate::client::{Client, ClientType, parse_basic_auth, validate_token_request};
use crate::config::AppState;
use crate::error::OAuthError;
use crate::jwt::keys::get_or_create_signing_key;
use crate::jwt::sign::{sign_access_token, sign_refresh_token};
use crate::jwt::verify::verify_refresh_token;
use crate::oauth::pkce::validate_pkce;
use crate::storage::{StorageAdapter, TransactOperation};

/// Token request form data
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
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

/// Stored authorization code data
#[derive(Debug, Deserialize)]
struct AuthCodeData {
    pub client_id: String,
    pub redirect_uri: String,
    pub subject: String,
    pub subject_type: String,
    pub properties: serde_json::Value,
    pub code_challenge: Option<String>,
    pub scope: Option<String>,
}

/// Handle the token request.
pub async fn handle_token<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Form(params): Form<TokenRequest>,
) -> Result<Json<TokenResponse>, OAuthError> {
    // Extract client_id from request body or Basic auth header
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let client_id = if let Some(id) = params.client_id.as_deref() {
        id.to_string()
    } else if let Some(header) = auth_header {
        let (id, _) = parse_basic_auth(Some(header))?;
        id
    } else {
        return Err(OAuthError::InvalidRequest("client_id required".to_string()));
    };

    // Validate client
    let client = validate_token_request(
        state.storage.as_ref(),
        &client_id,
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

async fn handle_authorization_code_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &Client,
) -> Result<Json<TokenResponse>, OAuthError> {
    let code = params
        .code
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("code required".to_string()))?;

    let redirect_uri = params
        .redirect_uri
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("redirect_uri required".to_string()))?;

    // Load authorization code from storage
    let code_value = state
        .storage
        .get(&["oauth:code", code])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidGrant("Invalid or expired authorization code".to_string()))?;

    // CRITICAL: Delete code BEFORE issuing tokens to prevent replay attacks
    state
        .storage
        .remove(&["oauth:code", code])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    let code_data: AuthCodeData = serde_json::from_value(code_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt auth code data: {}", e)))?;

    // Validate client_id matches
    if code_data.client_id != client.client_id {
        return Err(OAuthError::InvalidGrant("Code was not issued to this client".to_string()));
    }

    // Validate redirect_uri matches
    if code_data.redirect_uri != *redirect_uri {
        return Err(OAuthError::InvalidGrant("redirect_uri mismatch".to_string()));
    }

    // Validate PKCE
    if client.pkce_required {
        let verifier = params
            .code_verifier
            .as_ref()
            .ok_or_else(|| OAuthError::InvalidRequest("code_verifier required".to_string()))?;

        let challenge = code_data
            .code_challenge
            .as_ref()
            .ok_or_else(|| OAuthError::ServerError("Code missing challenge".to_string()))?;

        if !validate_pkce(verifier, challenge) {
            return Err(OAuthError::InvalidGrant("PKCE verification failed".to_string()));
        }
    }

    // Get signing key and issue tokens
    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://auth.example.com");

    let signing_key = get_or_create_signing_key(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    let access_ttl = state.config.tokens.access_token_ttl;
    let refresh_ttl = state.config.tokens.refresh_token_ttl;

    let access_token = sign_access_token(
        &signing_key,
        issuer,
        &client.client_id,
        &code_data.subject,
        &code_data.subject_type,
        code_data.properties,
        access_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;

    let refresh = sign_refresh_token(
        &signing_key,
        issuer,
        &client.client_id,
        &code_data.subject,
        refresh_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign refresh token: {}", e)))?;

    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        refresh_token: Some(refresh),
        scope: code_data.scope,
    }))
}

async fn handle_refresh_token_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &Client,
) -> Result<Json<TokenResponse>, OAuthError> {
    let refresh_token_str = params
        .refresh_token
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("refresh_token required".to_string()))?;

    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://auth.example.com");

    // Get all signing keys for verification (includes expired keys)
    let signing_keys = crate::jwt::keys::get_all_signing_keys(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    // Verify the refresh token
    let claims = verify_refresh_token(refresh_token_str, &signing_keys, issuer)
        .map_err(|e| OAuthError::InvalidGrant(format!("Invalid refresh token: {}", e)))?;

    // Verify audience matches client
    if claims.aud != client.client_id {
        return Err(OAuthError::InvalidGrant("Token was not issued to this client".to_string()));
    }

    // Get current signing key for new tokens
    let signing_key = get_or_create_signing_key(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    let access_ttl = state.config.tokens.access_token_ttl;
    let refresh_ttl = state.config.tokens.refresh_token_ttl;

    // Sign new tokens
    let new_access = sign_access_token(
        &signing_key,
        issuer,
        &client.client_id,
        &claims.sub,
        "user", // Refresh tokens don't carry subject_type, default to "user"
        serde_json::Value::Object(serde_json::Map::new()),
        access_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;

    let new_refresh = sign_refresh_token(
        &signing_key,
        issuer,
        &client.client_id,
        &claims.sub,
        refresh_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign refresh token: {}", e)))?;

    // Atomic rotation: delete old refresh token record, insert new one
    let old_key = vec!["oauth:refresh".to_string(), refresh_token_str.to_string()];
    let new_key = vec!["oauth:refresh".to_string(), new_refresh.clone()];
    let expiry = chrono::Utc::now() + chrono::Duration::seconds(refresh_ttl as i64);

    state
        .storage
        .transact(vec![
            TransactOperation::Delete { key: old_key },
            TransactOperation::Put {
                key: new_key,
                value: serde_json::json!({
                    "client_id": client.client_id,
                    "subject": claims.sub,
                }),
                expiry: Some(expiry),
            },
        ])
        .await
        .map_err(|e| OAuthError::ServerError(format!("Refresh token rotation failed: {}", e)))?;

    Ok(Json(TokenResponse {
        access_token: new_access,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        refresh_token: Some(new_refresh),
        scope: params.scope.clone(),
    }))
}

async fn handle_client_credentials_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &Client,
) -> Result<Json<TokenResponse>, OAuthError> {
    // Only confidential clients can use client_credentials
    if client.client_type != ClientType::Confidential {
        return Err(OAuthError::UnauthorizedClient(
            "Only confidential clients can use client_credentials".to_string(),
        ));
    }

    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://auth.example.com");

    let signing_key = get_or_create_signing_key(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    let access_ttl = state.config.tokens.access_token_ttl;

    // For client_credentials, the subject is the client itself
    let access_token = sign_access_token(
        &signing_key,
        issuer,
        &client.client_id,
        &client.client_id,
        "client",
        serde_json::Value::Object(serde_json::Map::new()),
        access_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;

    // No refresh token for client_credentials grant
    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        refresh_token: None,
        scope: params.scope.clone(),
    }))
}
