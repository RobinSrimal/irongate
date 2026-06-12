//! Code provider implementation.
//!
//! OTP-based authentication (email or SMS codes).
//! Storage keys:
//! - `["code:otp", destination_hash]` → OTP record with attempts counter

use crate::crypto::random::generate_unbiased_digits;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::traits::{Provider, ProviderContext, SubjectInfo};

/// Code provider configuration
#[derive(Debug, Clone)]
pub struct CodeConfig {
    /// Code length
    pub length: usize,
    /// Code expiry in seconds
    pub expiry: u64,
    /// Maximum verification attempts before the code is invalidated
    pub max_attempts: u32,
}

impl Default for CodeConfig {
    fn default() -> Self {
        Self {
            length: 6,
            expiry: 600, // 10 minutes
            max_attempts: 3,
        }
    }
}

/// Code provider (OTP)
pub struct CodeProvider {
    pub config: CodeConfig,
}

impl CodeProvider {
    pub fn new(config: CodeConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Provider for CodeProvider {
    fn name(&self) -> &str {
        "code"
    }

    fn provider_type(&self) -> &str {
        "code"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        _ctx: ProviderContext<S>,
    ) -> Router {
        // Code routes are handled by the functions below,
        // dispatched from the main router.
        router
    }
}

/// Stored OTP record
#[derive(Debug, Serialize, Deserialize)]
pub struct OtpRecord {
    pub destination: String,
    pub code_hash: String,
    pub attempts: u32,
    pub max_attempts: u32,
}

/// Hash a destination (email/phone) for use as a storage key.
fn hash_destination(destination: &str) -> String {
    let normalized = destination.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash an OTP code for storage (don't store plaintext codes).
fn hash_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hex::encode(hasher.finalize())
}

/// Request a new OTP code for a destination (email or phone).
///
/// Returns the plaintext code so the caller can deliver it (email/SMS).
pub async fn request_code<S: StorageAdapter>(
    storage: &S,
    destination: &str,
    config: &CodeConfig,
) -> Result<String, OAuthError> {
    if destination.is_empty() {
        return Err(OAuthError::InvalidRequest(
            "Destination is required".to_string(),
        ));
    }

    let dest_hash = hash_destination(destination);
    let code = generate_unbiased_digits(config.length);
    let code_hashed = hash_code(&code);

    let record = OtpRecord {
        destination: destination.to_string(),
        code_hash: code_hashed,
        attempts: 0,
        max_attempts: config.max_attempts,
    };

    let value = serde_json::to_value(&record)
        .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

    let expiry = Utc::now() + chrono::Duration::seconds(config.expiry as i64);
    storage
        .set(&["code:otp", &dest_hash], value, Some(expiry))
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(code)
}

/// Verify an OTP code.
///
/// Uses constant-time comparison to prevent timing attacks.
/// Enforces a maximum number of attempts.
pub async fn verify_code<S: StorageAdapter>(
    storage: &S,
    destination: &str,
    code: &str,
) -> Result<SubjectInfo, OAuthError> {
    let dest_hash = hash_destination(destination);

    let record_value = storage
        .get(&["code:otp", &dest_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| {
            OAuthError::InvalidGrant("No pending code for this destination".to_string())
        })?;

    let mut record: OtpRecord = serde_json::from_value(record_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt OTP record: {}", e)))?;

    // Check attempts
    if record.attempts >= record.max_attempts {
        // Delete the exhausted code
        let _ = storage.remove(&["code:otp", &dest_hash]).await;
        return Err(OAuthError::AccessDenied(
            "Maximum verification attempts exceeded".to_string(),
        ));
    }

    // Constant-time comparison of code hashes
    let provided_hash = hash_code(code);
    let stored_hash_bytes = record.code_hash.as_bytes();
    let provided_hash_bytes = provided_hash.as_bytes();

    let is_valid: bool = stored_hash_bytes.ct_eq(provided_hash_bytes).into();

    if !is_valid {
        // Increment attempts
        record.attempts += 1;
        let updated = serde_json::to_value(&record)
            .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

        // Re-store with same TTL (we don't extend expiry on failed attempts)
        storage
            .set(&["code:otp", &dest_hash], updated, None)
            .await
            .map_err(|e| OAuthError::ServerError(e.to_string()))?;

        return Err(OAuthError::InvalidGrant("Invalid code".to_string()));
    }

    // Code is valid — delete it (single use)
    storage
        .remove(&["code:otp", &dest_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(SubjectInfo {
        subject_type: "user".to_string(),
        properties: serde_json::json!({
            "destination": record.destination,
        }),
    })
}
