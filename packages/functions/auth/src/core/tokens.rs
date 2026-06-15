//! Token claim shapes for the target OAuth/OIDC core.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::scopes::{EMAIL, OPENID};
use crate::store::records::AuthorizationCodeRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub mode: String,
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub subject_type: String,
    pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenClaims {
    pub mode: String,
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}

pub fn scope_contains(scope: &str, expected: &str) -> bool {
    scope.split_whitespace().any(|part| part == expected)
}

pub fn build_access_token_claims(
    issuer: &str,
    audience: &str,
    code: &AuthorizationCodeRecord,
    ttl_seconds: u64,
) -> AccessTokenClaims {
    let now = Utc::now();
    let exp = now + Duration::seconds(ttl_seconds as i64);

    AccessTokenClaims {
        mode: "access".to_string(),
        iss: issuer.to_string(),
        sub: code.subject.clone(),
        aud: audience.to_string(),
        iat: now.timestamp(),
        exp: exp.timestamp(),
        scope: code.scope.clone(),
        subject_type: code.subject_type.clone(),
        properties: code.properties.clone(),
    }
}

pub fn build_id_token_claims(
    issuer: &str,
    client_id: &str,
    code: &AuthorizationCodeRecord,
    ttl_seconds: u64,
) -> Option<IdTokenClaims> {
    if !scope_contains(&code.scope, OPENID) {
        return None;
    }

    let now = Utc::now();
    let exp = now + Duration::seconds(ttl_seconds as i64);
    let include_email = scope_contains(&code.scope, EMAIL);
    let email = include_email
        .then(|| {
            code.properties
                .get("email")
                .and_then(|value| value.as_str())
        })
        .flatten()
        .map(ToString::to_string);
    let email_verified = include_email
        .then(|| {
            code.properties
                .get("email_verified")
                .and_then(|value| value.as_bool())
        })
        .flatten();

    Some(IdTokenClaims {
        mode: "id".to_string(),
        iss: issuer.to_string(),
        sub: code.subject.clone(),
        aud: client_id.to_string(),
        iat: now.timestamp(),
        exp: exp.timestamp(),
        nonce: code.oidc_nonce.clone(),
        email,
        email_verified,
    })
}
