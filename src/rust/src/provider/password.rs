//! Password provider implementation.
//!
//! Email/password authentication with verification codes.
//! Storage keys:
//! - `["password:user", email_hash]` → user record
//! - `["password:verify", code]` → verification code record
//! - `["password:reset", code]` → password reset code record

use crate::crypto::password::{hash_password, verify_password};
use crate::crypto::random::generate_unbiased_digits;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::traits::{Provider, ProviderContext, SubjectInfo};

/// Password provider configuration
#[derive(Debug, Clone)]
pub struct PasswordConfig {
    /// Whether email verification is required
    pub require_verification: bool,
    /// Code length for verification emails
    pub code_length: usize,
    /// Code expiry in seconds
    pub code_expiry: u64,
}

impl Default for PasswordConfig {
    fn default() -> Self {
        Self {
            require_verification: true,
            code_length: 6,
            code_expiry: 600, // 10 minutes
        }
    }
}

/// Password provider
pub struct PasswordProvider {
    pub config: PasswordConfig,
}

impl PasswordProvider {
    pub fn new(config: PasswordConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Provider for PasswordProvider {
    fn name(&self) -> &str {
        "password"
    }

    fn provider_type(&self) -> &str {
        "password"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        _ctx: ProviderContext<S>,
    ) -> Router {
        // Password routes are handled by the route handlers below,
        // dispatched from the main router.
        router
    }
}

/// Stored user record
#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordUser {
    pub email: String,
    pub password_hash: String,
    pub verified: bool,
    pub created_at: String,
}

/// Stored verification/reset code
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationCode {
    pub email: String,
    pub code_type: String, // "verify" or "reset"
}

/// Hash an email for use as a storage key (privacy-preserving).
fn hash_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// Register a new user with email and password.
pub async fn register<S: StorageAdapter>(
    storage: &S,
    email: &str,
    password: &str,
    config: &PasswordConfig,
) -> Result<RegisterResult, OAuthError> {
    // Validate inputs
    if email.is_empty() || !email.contains('@') {
        return Err(OAuthError::InvalidRequest("Invalid email address".to_string()));
    }
    if password.len() < 8 {
        return Err(OAuthError::InvalidRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let email_hash = hash_email(email);

    // Check if user already exists
    let existing = storage
        .get(&["password:user", &email_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    if existing.is_some() {
        return Err(OAuthError::InvalidRequest("Email already registered".to_string()));
    }

    // Hash password
    let password_hash = hash_password(password)
        .map_err(|e| OAuthError::ServerError(format!("Failed to hash password: {}", e)))?;

    let user = PasswordUser {
        email: email.to_string(),
        password_hash,
        verified: !config.require_verification,
        created_at: Utc::now().to_rfc3339(),
    };

    let user_value = serde_json::to_value(&user)
        .map_err(|e| OAuthError::ServerError(format!("Failed to serialize user: {}", e)))?;

    storage
        .set(&["password:user", &email_hash], user_value, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    // Generate verification code if required
    let verification_code = if config.require_verification {
        let code = generate_unbiased_digits(config.code_length);
        let code_record = VerificationCode {
            email: email.to_string(),
            code_type: "verify".to_string(),
        };
        let code_value = serde_json::to_value(&code_record)
            .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

        let expiry = Utc::now() + chrono::Duration::seconds(config.code_expiry as i64);
        storage
            .set(&["password:verify", &code], code_value, Some(expiry))
            .await
            .map_err(|e| OAuthError::ServerError(e.to_string()))?;

        Some(code)
    } else {
        None
    };

    Ok(RegisterResult {
        email: email.to_string(),
        verified: !config.require_verification,
        verification_code,
    })
}

/// Result of user registration
#[derive(Debug, Serialize)]
pub struct RegisterResult {
    pub email: String,
    pub verified: bool,
    /// The verification code (returned so the caller can send it via email).
    /// Only present when `require_verification` is true.
    pub verification_code: Option<String>,
}

/// Authenticate a user with email and password.
pub async fn login<S: StorageAdapter>(
    storage: &S,
    email: &str,
    password: &str,
    require_verified: bool,
) -> Result<SubjectInfo, OAuthError> {
    let email_hash = hash_email(email);

    let user_value = storage
        .get(&["password:user", &email_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::AccessDenied("Invalid email or password".to_string()))?;

    let user: PasswordUser = serde_json::from_value(user_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt user record: {}", e)))?;

    // Constant-time password verification
    if !verify_password(password, &user.password_hash) {
        return Err(OAuthError::AccessDenied("Invalid email or password".to_string()));
    }

    if require_verified && !user.verified {
        return Err(OAuthError::AccessDenied("Email not verified".to_string()));
    }

    Ok(SubjectInfo {
        subject_type: "user".to_string(),
        properties: serde_json::json!({
            "email": user.email,
            "verified": user.verified,
        }),
    })
}

/// Verify a user's email with a verification code.
pub async fn verify_email<S: StorageAdapter>(
    storage: &S,
    code: &str,
) -> Result<String, OAuthError> {
    // Look up code
    let code_value = storage
        .get(&["password:verify", code])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidGrant("Invalid or expired verification code".to_string()))?;

    let code_record: VerificationCode = serde_json::from_value(code_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt code record: {}", e)))?;

    // Delete the code (single use)
    storage
        .remove(&["password:verify", code])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    // Mark user as verified
    let email_hash = hash_email(&code_record.email);
    let user_value = storage
        .get(&["password:user", &email_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::ServerError("User not found".to_string()))?;

    let mut user: PasswordUser = serde_json::from_value(user_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt user record: {}", e)))?;

    user.verified = true;

    let updated = serde_json::to_value(&user)
        .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

    storage
        .set(&["password:user", &email_hash], updated, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(code_record.email)
}

/// Change a user's password (requires current password).
pub async fn change_password<S: StorageAdapter>(
    storage: &S,
    email: &str,
    current_password: &str,
    new_password: &str,
) -> Result<(), OAuthError> {
    if new_password.len() < 8 {
        return Err(OAuthError::InvalidRequest(
            "New password must be at least 8 characters".to_string(),
        ));
    }

    let email_hash = hash_email(email);

    let user_value = storage
        .get(&["password:user", &email_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::AccessDenied("User not found".to_string()))?;

    let mut user: PasswordUser = serde_json::from_value(user_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt user record: {}", e)))?;

    // Verify current password
    if !verify_password(current_password, &user.password_hash) {
        return Err(OAuthError::AccessDenied("Current password is incorrect".to_string()));
    }

    // Hash and store new password
    user.password_hash = hash_password(new_password)
        .map_err(|e| OAuthError::ServerError(format!("Failed to hash password: {}", e)))?;

    let updated = serde_json::to_value(&user)
        .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

    storage
        .set(&["password:user", &email_hash], updated, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(())
}

/// Initiate a password reset by generating a reset code.
pub async fn forgot_password<S: StorageAdapter>(
    storage: &S,
    email: &str,
    config: &PasswordConfig,
) -> Result<Option<String>, OAuthError> {
    let email_hash = hash_email(email);

    // Check if user exists (but don't reveal this to caller for security)
    let exists = storage
        .get(&["password:user", &email_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .is_some();

    if !exists {
        // Return None silently — don't reveal whether the email is registered
        return Ok(None);
    }

    let code = generate_unbiased_digits(config.code_length);
    let code_record = VerificationCode {
        email: email.to_string(),
        code_type: "reset".to_string(),
    };
    let code_value = serde_json::to_value(&code_record)
        .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

    let expiry = Utc::now() + chrono::Duration::seconds(config.code_expiry as i64);
    storage
        .set(&["password:reset", &code], code_value, Some(expiry))
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(Some(code))
}

/// Reset a password using a reset code.
pub async fn reset_password<S: StorageAdapter>(
    storage: &S,
    code: &str,
    new_password: &str,
) -> Result<String, OAuthError> {
    if new_password.len() < 8 {
        return Err(OAuthError::InvalidRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Look up reset code
    let code_value = storage
        .get(&["password:reset", code])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidGrant("Invalid or expired reset code".to_string()))?;

    let code_record: VerificationCode = serde_json::from_value(code_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt code record: {}", e)))?;

    // Delete code (single use)
    storage
        .remove(&["password:reset", code])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    // Update password
    let email_hash = hash_email(&code_record.email);
    let user_value = storage
        .get(&["password:user", &email_hash])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::ServerError("User not found".to_string()))?;

    let mut user: PasswordUser = serde_json::from_value(user_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt user record: {}", e)))?;

    user.password_hash = hash_password(new_password)
        .map_err(|e| OAuthError::ServerError(format!("Failed to hash password: {}", e)))?;

    let updated = serde_json::to_value(&user)
        .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;

    storage
        .set(&["password:user", &email_hash], updated, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(code_record.email)
}
