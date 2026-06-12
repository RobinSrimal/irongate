//! Well-known endpoints.
//!
//! Provides OAuth 2.0 metadata and JWKS endpoints.

use axum::{extract::State, response::Json};

use crate::config::AppState;
use crate::jwt::{get_all_signing_keys, to_jwks, Jwks};
use crate::storage::StorageAdapter;

/// OAuth 2.0 Authorization Server Metadata (RFC 8414)
#[derive(Debug, serde::Serialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
}

/// Handle /.well-known/oauth-authorization-server
pub async fn oauth_authorization_server<S: StorageAdapter>(
    State(state): State<AppState<S>>,
) -> Json<AuthorizationServerMetadata> {
    let base_url = state
        .config
        .issuer_url
        .clone()
        .unwrap_or_else(|| "https://localhost".to_string());

    Json(AuthorizationServerMetadata {
        issuer: base_url.clone(),
        authorization_endpoint: format!("{}/authorize", base_url),
        token_endpoint: format!("{}/token", base_url),
        userinfo_endpoint: format!("{}/userinfo", base_url),
        jwks_uri: format!("{}/.well-known/jwks.json", base_url),
        response_types_supported: vec!["code".to_string(), "token".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
            "client_credentials".to_string(),
        ],
        token_endpoint_auth_methods_supported: vec![
            "none".to_string(),
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
    })
}

/// Handle /.well-known/jwks.json
pub async fn jwks<S: StorageAdapter>(
    State(state): State<AppState<S>>,
) -> Result<Json<Jwks>, String> {
    let keys = get_all_signing_keys(state.storage.as_ref()).await?;
    Ok(Json(to_jwks(&keys)))
}
