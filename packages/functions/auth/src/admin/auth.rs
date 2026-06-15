//! Admin API authentication.
//!
//! Handles API key validation and bootstrap key generation.

use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::config::AppState;
use crate::crypto::random::generate_random_string;
use crate::error::AuthError;
use crate::storage::StorageAdapter;

/// Admin API key stored in DynamoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminKey {
    pub key_id: String,
    pub key_hash: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub permissions: Vec<String>,
}

/// Admin context after authentication
#[derive(Debug, Clone)]
pub struct AdminContext {
    pub key_id: String,
    pub permissions: Vec<String>,
}

/// Authenticate an admin request
pub async fn authenticate_admin_key<S: StorageAdapter>(
    state: &AppState<S>,
    api_key: &str,
) -> Result<AdminContext, AuthError> {
    // Parse key format: {key_id}:{secret}
    let (key_id, provided_secret) = api_key
        .split_once(':')
        .ok_or(AuthError::InvalidKeyFormat)?;

    // Fetch key from storage
    let key_data: AdminKey = state
        .storage
        .get(&["admin:key", key_id])
        .await
        .map_err(|_| AuthError::InvalidApiKey)?
        .ok_or(AuthError::InvalidApiKey)
        .and_then(|v| serde_json::from_value(v).map_err(|_| AuthError::InvalidApiKey))?;

    // Verify secret using constant-time comparison
    let provided_hash = sha256_hex(provided_secret);
    let expected_hash = &key_data.key_hash;

    if !bool::from(provided_hash.as_bytes().ct_eq(expected_hash.as_bytes())) {
        return Err(AuthError::InvalidApiKey);
    }

    Ok(AdminContext {
        key_id: key_id.to_string(),
        permissions: key_data.permissions,
    })
}

/// Check if an admin context has a required permission.
pub fn has_permission(ctx: &AdminContext, required: &str) -> bool {
    ctx.permissions.iter().any(|perm| {
        if perm == "*" {
            return true;
        }
        if perm == required {
            return true;
        }
        if let Some(prefix) = perm.strip_suffix('*') {
            return required.starts_with(prefix);
        }
        false
    })
}

/// Require a permission or return an AuthError.
pub fn require_permission(ctx: &AdminContext, required: &str) -> Result<(), AuthError> {
    if has_permission(ctx, required) {
        Ok(())
    } else {
        Err(AuthError::InsufficientPermissions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{environment::RuntimeAuthConfig, AppState, Config, ProviderConfig};
    use crate::email::NoopEmailSender;
    use crate::storage::test_support::TestStorage;
    use std::collections::HashMap;
    use std::sync::Arc;
    use axum::http::HeaderMap;

    #[test]
    fn permission_allows_exact_and_wildcards() {
        let ctx = AdminContext {
            key_id: "k1".to_string(),
            permissions: vec!["clients:read".to_string(), "tokens:*".to_string()],
        };

        assert!(has_permission(&ctx, "clients:read"));
        assert!(!has_permission(&ctx, "clients:write"));
        assert!(has_permission(&ctx, "tokens:revoke"));
        assert!(has_permission(&ctx, "tokens:anything"));
    }

    #[test]
    fn permission_allows_global_star() {
        let ctx = AdminContext {
            key_id: "k2".to_string(),
            permissions: vec!["*".to_string()],
        };

        assert!(has_permission(&ctx, "clients:write"));
        assert!(has_permission(&ctx, "tokens:revoke"));
    }

    #[tokio::test]
    async fn authenticate_admin_key_roundtrip() {
        let storage = TestStorage::new();
        let config = Config::dev();
        let state = AppState {
            storage: Arc::new(storage),
            config: Arc::new(config),
            runtime: Arc::new(RuntimeAuthConfig::for_tests()),
            providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
            email_sender: Arc::new(NoopEmailSender::default()),
            google_client: Arc::new(crate::providers::google::ReqwestGoogleOidcClient::new()),
        };

        // Bootstrap a key
        let response = bootstrap(axum::extract::State(state.clone()))
            .await
            .unwrap();
        let api_key = response.0.api_key.clone();

        // Extract from headers
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Admin-API-Key",
            api_key.parse().unwrap(),
        );
        let extracted = extract_api_key(&headers).unwrap();
        let ctx = authenticate_admin_key(&state, &extracted).await.unwrap();
        assert_eq!(ctx.permissions, vec!["*".to_string()]);
    }
}

/// Extract API key from request headers
pub fn extract_api_key(headers: &HeaderMap) -> Result<String, AuthError> {
    // Try X-Admin-API-Key header first
    if let Some(key) = headers.get("X-Admin-API-Key") {
        return key
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| AuthError::InvalidKeyFormat);
    }

    // Fall back to Authorization: Bearer
    if let Some(auth) = headers.get("Authorization") {
        let auth_str = auth.to_str().map_err(|_| AuthError::InvalidKeyFormat)?;
        if let Some(key) = auth_str.strip_prefix("Bearer ") {
            return Ok(key.to_string());
        }
    }

    Err(AuthError::MissingApiKey)
}

/// Compute SHA-256 hash as hex string
fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Bootstrap response
#[derive(Debug, Serialize)]
pub struct BootstrapResponse {
    pub api_key: String,
    pub message: String,
}

/// Bootstrap the initial admin API key
///
/// This can only be called once - subsequent calls will fail.
pub async fn bootstrap<S: StorageAdapter>(
    State(state): State<AppState<S>>,
) -> Result<Json<BootstrapResponse>, AuthError> {
    // Check if any admin key exists
    let existing = state
        .storage
        .scan(&["admin:key"])
        .await
        .map_err(|_| AuthError::InvalidApiKey)?;

    if !existing.is_empty() {
        return Err(AuthError::InsufficientPermissions);
    }

    // Generate new admin key
    let key_id = generate_random_string(16);
    let secret = generate_random_string(32);
    let key_hash = sha256_hex(&secret);

    let admin_key = AdminKey {
        key_id: key_id.clone(),
        key_hash,
        name: "bootstrap".to_string(),
        created_at: chrono::Utc::now(),
        permissions: vec!["*".to_string()], // Full access
    };

    // Store the key
    let value = serde_json::to_value(&admin_key).map_err(|_| AuthError::InvalidApiKey)?;
    state
        .storage
        .set(&["admin:key", &key_id], value, None)
        .await
        .map_err(|_| AuthError::InvalidApiKey)?;

    // Return full key (only shown once)
    let full_key = format!("{}:{}", key_id, secret);

    Ok(Json(BootstrapResponse {
        api_key: full_key,
        message: "Admin API key created. Save this key - it will not be shown again!".to_string(),
    }))
}
