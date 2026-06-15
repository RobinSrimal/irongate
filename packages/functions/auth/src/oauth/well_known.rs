//! Well-known endpoints.
//!
//! Provides OAuth 2.0 metadata and JWKS endpoints.

use axum::{extract::State, response::Json};

use crate::config::AppState;
use crate::core::scopes::DEFAULT_SUPPORTED_SCOPES;
use crate::jwt::{get_all_signing_keys, to_jwks, Jwks};
use crate::storage::StorageAdapter;

/// OAuth 2.0 Authorization Server Metadata (RFC 8414)
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub revocation_endpoint: String,
    pub jwks_uri: String,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub claims_supported: Vec<String>,
}

pub fn build_authorization_server_metadata(issuer: &str) -> AuthorizationServerMetadata {
    let base_url = issuer.trim_end_matches('/').to_string();

    AuthorizationServerMetadata {
        issuer: base_url.clone(),
        authorization_endpoint: format!("{}/authorize", base_url),
        token_endpoint: format!("{}/token", base_url),
        userinfo_endpoint: format!("{}/userinfo", base_url),
        revocation_endpoint: format!("{}/oauth/revoke", base_url),
        jwks_uri: format!("{}/.well-known/jwks.json", base_url),
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        scopes_supported: DEFAULT_SUPPORTED_SCOPES
            .iter()
            .map(|scope| (*scope).to_string())
            .collect(),
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec!["ES256".to_string()],
        token_endpoint_auth_methods_supported: vec![
            "none".to_string(),
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
        claims_supported: vec![
            "sub".to_string(),
            "iss".to_string(),
            "aud".to_string(),
            "exp".to_string(),
            "iat".to_string(),
            "email".to_string(),
            "email_verified".to_string(),
        ],
    }
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

    Json(build_authorization_server_metadata(&base_url))
}

/// Handle /.well-known/openid-configuration
pub async fn openid_configuration<S: StorageAdapter>(
    State(state): State<AppState<S>>,
) -> Json<AuthorizationServerMetadata> {
    let base_url = state
        .config
        .issuer_url
        .clone()
        .unwrap_or_else(|| "https://localhost".to_string());

    Json(build_authorization_server_metadata(&base_url))
}

/// Handle /.well-known/jwks.json
pub async fn jwks<S: StorageAdapter>(
    State(state): State<AppState<S>>,
) -> Result<Json<Jwks>, String> {
    let keys = get_all_signing_keys(state.storage.as_ref()).await?;
    Ok(Json(to_jwks(&keys)))
}
