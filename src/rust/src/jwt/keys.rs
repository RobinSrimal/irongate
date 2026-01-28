//! Key management for JWT signing.
//!
//! Handles ES256 key generation, storage, and rotation.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::crypto::random::generate_uuid;
use crate::storage::StorageAdapter;

/// Signing key stored in DynamoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKey {
    /// Unique key identifier (UUID)
    pub kid: String,
    /// Private key in PEM format
    pub private_key_pem: String,
    /// Public key in PEM format
    pub public_key_pem: String,
    /// When the key was created
    pub created_at: DateTime<Utc>,
    /// When the key expires (for rotation)
    pub expires_at: DateTime<Utc>,
}

/// JWKS (JSON Web Key Set) response
#[derive(Debug, Serialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// Individual JWK (JSON Web Key)
#[derive(Debug, Serialize)]
pub struct Jwk {
    pub kty: String,
    pub alg: String,
    pub use_: String,
    pub kid: String,
    pub crv: String,
    pub x: String,
    pub y: String,
}

/// Generate a new ES256 signing key pair.
pub fn generate_signing_key() -> Result<SigningKey, String> {
    todo!("Implement ES256 key generation using p256 crate")
}

/// Get the current signing key, generating one if needed.
pub async fn get_or_create_signing_key<S: StorageAdapter>(
    storage: &S,
) -> Result<SigningKey, String> {
    // Look for an existing, non-expired key
    let keys = storage
        .scan(&["signing:key"])
        .await
        .map_err(|e| e.to_string())?;

    let now = Utc::now();
    for (_, value) in keys {
        if let Ok(key) = serde_json::from_value::<SigningKey>(value) {
            if key.expires_at > now {
                return Ok(key);
            }
        }
    }

    // No valid key found, generate a new one
    let key = generate_signing_key()?;
    let value = serde_json::to_value(&key).map_err(|e| e.to_string())?;

    storage
        .set(&["signing:key", &key.kid], value, None)
        .await
        .map_err(|e| e.to_string())?;

    Ok(key)
}

/// Get all signing keys for JWKS endpoint.
///
/// Returns all keys (including expired) for token verification.
pub async fn get_all_signing_keys<S: StorageAdapter>(
    storage: &S,
) -> Result<Vec<SigningKey>, String> {
    let keys = storage
        .scan(&["signing:key"])
        .await
        .map_err(|e| e.to_string())?;

    let mut signing_keys = Vec::new();
    for (_, value) in keys {
        if let Ok(key) = serde_json::from_value::<SigningKey>(value) {
            signing_keys.push(key);
        }
    }

    // Sort by creation date (newest first)
    signing_keys.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(signing_keys)
}

/// Convert signing keys to JWKS format.
pub fn to_jwks(keys: &[SigningKey]) -> Jwks {
    todo!("Implement conversion to JWKS format")
}
