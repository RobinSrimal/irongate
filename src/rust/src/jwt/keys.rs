//! Key management for JWT signing.
//!
//! Handles ES256 key generation, storage, and rotation.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use p256::ecdsa::SigningKey as P256SigningKey;
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
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
    let signing_key = P256SigningKey::random(&mut rand::rngs::OsRng);

    let private_key_pem = signing_key
        .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| format!("Failed to encode private key: {e}"))?
        .to_string();

    let verifying_key = signing_key.verifying_key();
    let public_key_pem = verifying_key
        .to_public_key_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| format!("Failed to encode public key: {e}"))?;

    let now = Utc::now();
    Ok(SigningKey {
        kid: generate_uuid(),
        private_key_pem,
        public_key_pem,
        created_at: now,
        expires_at: now + Duration::days(90),
    })
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
    let jwk_keys: Vec<Jwk> = keys
        .iter()
        .filter_map(|key| {
            let signing_key = P256SigningKey::from_pkcs8_pem(&key.private_key_pem).ok()?;
            let verifying_key = signing_key.verifying_key();
            let point = verifying_key.to_encoded_point(false);

            let x = URL_SAFE_NO_PAD.encode(point.x()?);
            let y = URL_SAFE_NO_PAD.encode(point.y()?);

            Some(Jwk {
                kty: "EC".to_string(),
                alg: "ES256".to_string(),
                use_: "sig".to_string(),
                kid: key.kid.clone(),
                crv: "P-256".to_string(),
                x,
                y,
            })
        })
        .collect();

    Jwks { keys: jwk_keys }
}
