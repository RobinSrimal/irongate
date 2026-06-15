//! Typed records stored by the target auth store facade.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Active,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityStatus {
    Active,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub subject: String,
    pub status: AccountStatus,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub provider: String,
    pub identity_digest: String,
    pub subject: String,
    pub status: IdentityStatus,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub reusable_after: Option<DateTime<Utc>>,
    pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordUserRecord {
    pub email: String,
    pub subject: Option<String>,
    pub password_hash: String,
    pub password_hash_updated_at: DateTime<Utc>,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerificationRecord {
    pub email_digest: String,
    pub purpose: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetRecord {
    pub email_digest: String,
    pub subject: String,
    pub purpose: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeSessionRecord {
    pub client_id: String,
    pub redirect_uri: String,
    pub state: Option<String>,
    pub scope: String,
    pub oidc_nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub selected_provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationCodeRecord {
    pub client_id: String,
    pub redirect_uri: String,
    pub subject: String,
    pub subject_type: String,
    pub properties: Value,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub scope: String,
    pub oidc_nonce: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStateRecord {
    pub session_lookup_digest: String,
    pub provider: String,
    pub pkce_verifier: String,
    pub nonce: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeSecretRecord {
    pub family: String,
    pub lookup_digest: String,
    pub subject: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub attempts: u32,
    pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRecord {
    pub refresh_digest: String,
    pub family_id: String,
    pub client_id: String,
    pub subject: String,
    pub subject_type: String,
    pub scope: String,
    pub properties: Value,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub replaced_by: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenFamilyRecord {
    pub family_id: String,
    pub client_id: String,
    pub subject: String,
    pub subject_type: String,
    pub scope: String,
    pub properties: Value,
    pub current_refresh_digest: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenIndexRecord {
    pub refresh_digest: String,
    pub family_id: String,
    pub client_id: String,
    pub subject: String,
    pub expires_at: DateTime<Utc>,
}
