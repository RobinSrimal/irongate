//! JWT signing.
//!
//! Signs tokens using ES256 (ECDSA with P-256).

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use super::keys::SigningKey;

/// JWT claims for access tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    /// Token mode (always "access" for access tokens)
    pub mode: String,
    /// Subject type (e.g., "user", "account")
    #[serde(rename = "type")]
    pub subject_type: String,
    /// Subject properties
    pub properties: serde_json::Value,
    /// Audience (client_id)
    pub aud: String,
    /// Issuer URL
    pub iss: String,
    /// Subject identifier
    pub sub: String,
    /// Expiration timestamp
    pub exp: i64,
    /// Issued at timestamp
    pub iat: i64,
}

/// JWT claims for refresh tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    /// Token mode (always "refresh" for refresh tokens)
    pub mode: String,
    /// Subject identifier
    pub sub: String,
    /// Audience (client_id)
    pub aud: String,
    /// Issuer URL
    pub iss: String,
    /// Expiration timestamp
    pub exp: i64,
    /// Issued at timestamp
    pub iat: i64,
}

/// Sign an access token.
pub fn sign_access_token(
    signing_key: &SigningKey,
    issuer: &str,
    audience: &str,
    subject: &str,
    subject_type: &str,
    properties: serde_json::Value,
    ttl_seconds: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = now + Duration::seconds(ttl_seconds as i64);

    let claims = AccessTokenClaims {
        mode: "access".to_string(),
        subject_type: subject_type.to_string(),
        properties,
        aud: audience.to_string(),
        iss: issuer.to_string(),
        sub: subject.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(signing_key.kid.clone());

    let key = EncodingKey::from_ec_pem(signing_key.private_key_pem.as_bytes())?;
    encode(&header, &claims, &key)
}

/// Sign a refresh token.
pub fn sign_refresh_token(
    signing_key: &SigningKey,
    issuer: &str,
    audience: &str,
    subject: &str,
    ttl_seconds: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = now + Duration::seconds(ttl_seconds as i64);

    let claims = RefreshTokenClaims {
        mode: "refresh".to_string(),
        sub: subject.to_string(),
        aud: audience.to_string(),
        iss: issuer.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(signing_key.kid.clone());

    let key = EncodingKey::from_ec_pem(signing_key.private_key_pem.as_bytes())?;
    encode(&header, &claims, &key)
}
