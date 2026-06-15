//! Well-known endpoints.
//!
//! Provides OAuth 2.0 metadata and JWKS endpoints.

use axum::{extract::State, response::Json};

use crate::config::AppState;
use crate::core::scopes::DEFAULT_SUPPORTED_SCOPES;
use crate::crypto::signing::Jwks;

/// OAuth 2.0 Authorization Server Metadata (RFC 8414)
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub revocation_endpoint: String,
    pub userinfo_endpoint: String,
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
        revocation_endpoint: format!("{}/oauth/revoke", base_url),
        userinfo_endpoint: format!("{}/userinfo", base_url),
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
pub async fn oauth_authorization_server(
    State(state): State<AppState>,
) -> Json<AuthorizationServerMetadata> {
    let base_url = state
        .config
        .issuer_url
        .clone()
        .unwrap_or_else(|| "https://localhost".to_string());

    Json(build_authorization_server_metadata(&base_url))
}

/// Handle /.well-known/openid-configuration
pub async fn openid_configuration(
    State(state): State<AppState>,
) -> Json<AuthorizationServerMetadata> {
    let base_url = state
        .config
        .issuer_url
        .clone()
        .unwrap_or_else(|| "https://localhost".to_string());

    Json(build_authorization_server_metadata(&base_url))
}

/// Handle /.well-known/jwks.json
pub async fn jwks(State(state): State<AppState>) -> Result<Json<Jwks>, String> {
    Ok(Json(state.runtime.signer.jwks()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_metadata_advertises_target_oauth_flows() {
        let metadata = build_authorization_server_metadata("https://auth.example.com");

        assert_eq!(metadata.issuer, "https://auth.example.com");
        assert_eq!(
            metadata.grant_types_supported,
            vec![
                "authorization_code".to_string(),
                "refresh_token".to_string()
            ]
        );
        assert!(metadata
            .scopes_supported
            .contains(&"offline_access".to_string()));
        assert_eq!(metadata.response_types_supported, vec!["code".to_string()]);
        assert_eq!(
            metadata.id_token_signing_alg_values_supported,
            vec!["ES256".to_string()]
        );
        let metadata_json = serde_json::to_value(&metadata).expect("metadata json");
        assert_eq!(
            metadata_json["revocation_endpoint"],
            "https://auth.example.com/oauth/revoke"
        );
        assert!(!metadata
            .grant_types_supported
            .contains(&"client_credentials".to_string()));
    }
}
