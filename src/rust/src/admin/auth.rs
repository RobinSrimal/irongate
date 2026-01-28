//! Admin API authentication.
//!
//! Handles API key validation and bootstrap key generation.

use axum::{extract::State, http::Request, Json};
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
pub async fn authenticate_admin_request<S: StorageAdapter, B>(
    state: &AppState<S>,
    req: &Request<B>,
) -> Result<AdminContext, AuthError> {
    // Get API key from header
    let api_key = extract_api_key(req)?;

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

/// Extract API key from request headers
fn extract_api_key<B>(req: &Request<B>) -> Result<&str, AuthError> {
    // Try X-Admin-API-Key header first
    if let Some(key) = req.headers().get("X-Admin-API-Key") {
        return key.to_str().map_err(|_| AuthError::InvalidKeyFormat);
    }

    // Fall back to Authorization: Bearer
    if let Some(auth) = req.headers().get("Authorization") {
        let auth_str = auth.to_str().map_err(|_| AuthError::InvalidKeyFormat)?;
        if let Some(key) = auth_str.strip_prefix("Bearer ") {
            return Ok(key);
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
