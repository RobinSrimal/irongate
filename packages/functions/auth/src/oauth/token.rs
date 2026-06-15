//! Token endpoint (/token).
//!
//! Handles authorization-code exchange and refresh-token rotation.

use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
    Form,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::audit::{self, AuditEvent};
use crate::client::parse_basic_auth;
use crate::config::{AppState, Endpoint};
use crate::core::clients::{ClientType, ConfiguredClient, GrantType, TokenEndpointAuthMethod};
use crate::core::scopes::OFFLINE_ACCESS;
use crate::core::subjects::Subject;
use crate::core::tokens::{
    build_access_token_claims, build_access_token_claims_from_refresh, build_id_token_claims,
    scope_contains,
};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::error::{OAuthError, StorageError};
use crate::jwt::keys::get_or_create_signing_key;
use crate::jwt::sign::{sign_access_token, sign_refresh_token};
use crate::jwt::verify::verify_refresh_token;
use crate::oauth::pkce::validate_pkce;
use crate::ratelimit::middleware::{
    check_rate_limit, extract_client_ip, get_rate_limit_identifier,
};
use crate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use crate::store::records::RefreshTokenRecord as StoreRefreshTokenRecord;
use crate::store::refresh::{CreateRefreshTokenInput, RefreshTokenStoreError};
use crate::store::AuthStore;

/// Token request form data
#[derive(Debug, Deserialize, Clone)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// Token response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Stored refresh token record (for revocation/rotation)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct RefreshTokenRecord {
    pub client_id: String,
    pub subject: String,
    #[serde(default)]
    pub issued_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_used_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub replaced_by: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Handle the token request.
pub async fn handle_token<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Form(params): Form<TokenRequest>,
) -> Result<Response, OAuthError> {
    // Extract client_id from request body or Basic auth header
    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());

    let client_id = if let Some(id) = params.client_id.as_deref() {
        id.to_string()
    } else if let Some(header) = auth_header {
        let (id, _) = parse_basic_auth(Some(header))?;
        id
    } else {
        return Err(OAuthError::InvalidRequest("client_id required".to_string()));
    };

    // Rate limit by client_id (or IP as fallback)
    let ip = extract_client_ip(&headers, &state.config.proxy);
    let identifier = get_rate_limit_identifier(Some(&client_id), ip.as_deref());
    if let Err(err) = check_rate_limit(
        state.storage.as_ref(),
        &state.config.rate_limit,
        Endpoint::Token,
        &identifier,
    )
    .await
    {
        return Ok(err.into_response());
    }

    let grant = match params.grant_type.as_str() {
        "authorization_code" => GrantType::AuthorizationCode,
        "refresh_token" => GrantType::RefreshToken,
        "client_credentials" => {
            return Err(OAuthError::UnsupportedGrantType(
                "client_credentials".to_string(),
            ))
        }
        _ => return Err(OAuthError::UnsupportedGrantType(params.grant_type)),
    };

    let configured = state
        .runtime
        .client_registry
        .get(&client_id)
        .ok_or_else(|| OAuthError::InvalidClient("Client not registered".to_string()))?;
    let basic_secret;
    let provided_secret = match configured.client_type {
        ClientType::Public => None,
        ClientType::Confidential => match configured.token_endpoint_auth_method {
            TokenEndpointAuthMethod::None => {
                return Err(OAuthError::InvalidClient(
                    "Confidential client must have auth method".to_string(),
                ))
            }
            TokenEndpointAuthMethod::ClientSecretPost => params.client_secret.as_deref(),
            TokenEndpointAuthMethod::ClientSecretBasic => {
                let (basic_client_id, secret) = parse_basic_auth(auth_header)?;
                if basic_client_id != client_id {
                    return Err(OAuthError::InvalidClient(
                        "Basic auth client_id mismatch".to_string(),
                    ));
                }
                basic_secret = Some(secret);
                basic_secret.as_deref()
            }
        },
    };

    let client = state
        .runtime
        .client_registry
        .validate_token_request(&client_id, grant, provided_secret)
        .map_err(OAuthError::from)?;

    // Handle based on grant type
    let response = match params.grant_type.as_str() {
        "authorization_code" => handle_authorization_code_grant(&state, &params, client).await?,
        "refresh_token" => handle_target_refresh_token_grant(&state, &params, client, ip).await?,
        _ => return Err(OAuthError::UnsupportedGrantType(params.grant_type)),
    };

    Ok(Json(response).into_response())
}

async fn handle_authorization_code_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &ConfiguredClient,
) -> Result<TokenResponse, OAuthError> {
    let code = params
        .code
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("code required".to_string()))?;

    let redirect_uri = params
        .redirect_uri
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("redirect_uri required".to_string()))?;

    let code_digest = lookup_digest(
        state.runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizationCode,
        code,
    );
    let store = AuthStore::new(state.storage.clone());
    let code_data = store
        .take_authorization_code(&code_digest)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| {
            OAuthError::InvalidGrant("Invalid or expired authorization code".to_string())
        })?;

    // Validate client_id matches
    if code_data.client_id != client.client_id {
        return Err(OAuthError::InvalidGrant(
            "Code was not issued to this client".to_string(),
        ));
    }

    // Validate redirect_uri matches
    if code_data.redirect_uri != *redirect_uri {
        return Err(OAuthError::InvalidGrant(
            "redirect_uri mismatch".to_string(),
        ));
    }

    if code_data.code_challenge_method.as_deref() != Some("S256") {
        return Err(OAuthError::InvalidGrant(
            "authorization code is missing supported PKCE method".to_string(),
        ));
    }

    // Validate PKCE
    if client.pkce_required {
        let verifier = params
            .code_verifier
            .as_ref()
            .ok_or_else(|| OAuthError::InvalidRequest("code_verifier required".to_string()))?;

        let challenge = code_data
            .code_challenge
            .as_ref()
            .ok_or_else(|| OAuthError::ServerError("Code missing challenge".to_string()))?;

        if !validate_pkce(verifier, challenge) {
            return Err(OAuthError::InvalidGrant(
                "PKCE verification failed".to_string(),
            ));
        }
    }

    let subject = Subject::from_persisted(code_data.subject.clone());
    if !store
        .is_active_account(&subject)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
    {
        return Err(OAuthError::InvalidGrant(
            "subject account is not active".to_string(),
        ));
    }

    // Issue runtime-signed tokens.
    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://localhost");
    let access_ttl = state.runtime.ttls.access_token_seconds;
    let id_ttl = state.runtime.ttls.id_token_seconds;
    let access_claims = build_access_token_claims(
        issuer,
        &state.runtime.access_token_audience,
        &code_data,
        access_ttl,
    );
    let access_token = state
        .runtime
        .signer
        .sign_access_token(&access_claims)
        .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;
    let id_token = build_id_token_claims(issuer, &client.client_id, &code_data, id_ttl)
        .map(|claims| state.runtime.signer.sign_id_token(&claims))
        .transpose()
        .map_err(|e| OAuthError::ServerError(format!("Failed to sign ID token: {}", e)))?;
    let refresh_token = if scope_contains(&code_data.scope, OFFLINE_ACCESS) {
        if !client.allowed_grant_types.contains(&GrantType::RefreshToken) {
            return Err(OAuthError::UnauthorizedClient(
                "client is not allowed to receive refresh tokens".to_string(),
            ));
        }
        let refresh_expires_at = Utc::now()
            + Duration::seconds(state.runtime.ttls.refresh_token_seconds as i64);
        let created = store
            .create_refresh_token(
                state.runtime.lookup_secret.as_bytes(),
                CreateRefreshTokenInput {
                    client_id: client.client_id.clone(),
                    subject: code_data.subject.clone(),
                    subject_type: code_data.subject_type.clone(),
                    scope: code_data.scope.clone(),
                    properties: code_data.properties.clone(),
                    expires_at: refresh_expires_at,
                },
            )
            .await
            .map_err(|e| OAuthError::ServerError(e.to_string()))?;

        let mut refresh_event = AuditEvent::new("refresh_token_issued");
        refresh_event.client_id = Some(client.client_id.clone());
        refresh_event.subject = Some(code_data.subject.clone());
        refresh_event.token_hash = Some(created.refresh_digest);
        let _ = audit::record_event(state.storage.as_ref(), refresh_event).await;

        Some(created.raw_token)
    } else {
        None
    };

    let mut event = AuditEvent::new("authorization_code_exchanged");
    event.client_id = Some(client.client_id.clone());
    event.subject = Some(code_data.subject.clone());
    let _ = audit::record_event(state.storage.as_ref(), event).await;

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        id_token,
        refresh_token,
        scope: Some(code_data.scope),
    })
}

async fn handle_target_refresh_token_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &ConfiguredClient,
    ip: Option<String>,
) -> Result<TokenResponse, OAuthError> {
    let refresh_token_str = params
        .refresh_token
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("refresh_token required".to_string()))?;
    let refresh_digest = lookup_digest(
        state.runtime.lookup_secret.as_bytes(),
        LookupFamily::RefreshToken,
        refresh_token_str,
    );
    let store = AuthStore::new(state.storage.clone());
    let refresh_expires_at =
        Utc::now() + Duration::seconds(state.runtime.ttls.refresh_token_seconds as i64);

    let rotated = match store
        .rotate_refresh_token(
            state.runtime.lookup_secret.as_bytes(),
            refresh_token_str,
            &client.client_id,
            refresh_expires_at,
        )
        .await
    {
        Ok(rotated) => rotated,
        Err(RefreshTokenStoreError::Invalid)
        | Err(RefreshTokenStoreError::WrongClient)
        | Err(RefreshTokenStoreError::SubjectInactive) => {
            return Err(OAuthError::InvalidGrant(
                "Refresh token revoked or expired".to_string(),
            ))
        }
        Err(RefreshTokenStoreError::ReuseDetected) => {
            let mut event = AuditEvent::new("refresh_token_reuse");
            event.client_id = Some(client.client_id.clone());
            event.token_hash = Some(refresh_digest);
            event.ip = ip;
            event.detail = Some("token already rotated or revoked".to_string());
            let _ = audit::record_event(state.storage.as_ref(), event).await;
            return Err(OAuthError::InvalidGrant(
                "Refresh token revoked or expired".to_string(),
            ));
        }
        Err(RefreshTokenStoreError::Storage(err)) => {
            return Err(OAuthError::ServerError(err.to_string()));
        }
    };

    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://localhost");
    let access_ttl = state.runtime.ttls.access_token_seconds;
    let refresh_record = StoreRefreshTokenRecord {
        refresh_digest: rotated.refresh_digest.clone(),
        family_id: rotated.family_id.clone(),
        client_id: rotated.client_id.clone(),
        subject: rotated.subject.clone(),
        subject_type: rotated.subject_type.clone(),
        scope: rotated.scope.clone(),
        properties: rotated.properties.clone(),
        issued_at: Utc::now(),
        expires_at: rotated.expires_at,
        last_used_at: None,
        replaced_by: None,
        revoked_at: None,
    };
    let access_claims = build_access_token_claims_from_refresh(
        issuer,
        &state.runtime.access_token_audience,
        &refresh_record,
        access_ttl,
    );
    let access_token = state
        .runtime
        .signer
        .sign_access_token(&access_claims)
        .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;

    let mut event = AuditEvent::new("refresh_token_rotated");
    event.client_id = Some(client.client_id.clone());
    event.subject = Some(rotated.subject.clone());
    event.token_hash = Some(rotated.refresh_digest.clone());
    event.ip = ip;
    let _ = audit::record_event(state.storage.as_ref(), event).await;

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        id_token: None,
        refresh_token: Some(rotated.raw_token),
        scope: Some(rotated.scope),
    })
}

async fn handle_refresh_token_grant<S: StorageAdapter>(
    state: &AppState<S>,
    params: &TokenRequest,
    client: &ConfiguredClient,
    ip: Option<String>,
) -> Result<TokenResponse, OAuthError> {
    let refresh_token_str = params
        .refresh_token
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("refresh_token required".to_string()))?;

    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://auth.example.com");

    // Get all signing keys for verification (includes expired keys)
    let signing_keys = crate::jwt::keys::get_all_signing_keys(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    // Verify the refresh token
    let claims = verify_refresh_token(refresh_token_str, &signing_keys, issuer)
        .map_err(|e| OAuthError::InvalidGrant(format!("Invalid refresh token: {}", e)))?;

    // Verify audience matches client
    if claims.aud != client.client_id {
        return Err(OAuthError::InvalidGrant("Token was not issued to this client".to_string()));
    }

    // Verify refresh token exists and matches stored record (revocation/rotation)
    let record_value = state
        .storage
        .get(&["oauth:refresh", refresh_token_str])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| {
            OAuthError::InvalidGrant("Refresh token revoked or expired".to_string())
        })?;

    let record: RefreshTokenRecord = serde_json::from_value(record_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt refresh record: {}", e)))?;

    if record.revoked_at.is_some() || record.replaced_by.is_some() {
        let _ = log_refresh_event(
            state.storage.as_ref(),
            "refresh_token_reuse",
            &record,
            refresh_token_str,
            ip.as_deref(),
            Some("token already revoked or rotated"),
        )
        .await;
        let _ = revoke_refresh_tokens(
            state.storage.as_ref(),
            Some(&record.subject),
            Some(&record.client_id),
        )
        .await;
        return Err(OAuthError::InvalidGrant("Refresh token revoked or expired".to_string()));
    }

    if record.client_id != client.client_id || record.subject != claims.sub {
        return Err(OAuthError::InvalidGrant(
            "Refresh token does not match client".to_string(),
        ));
    }

    // Get current signing key for new tokens
    let signing_key = get_or_create_signing_key(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    let access_ttl = state.config.tokens.access_token_ttl;
    let refresh_ttl = state.config.tokens.refresh_token_ttl;

    // Sign new tokens
    let new_access = sign_access_token(
        &signing_key,
        issuer,
        &client.client_id,
        &claims.sub,
        "user", // Refresh tokens don't carry subject_type, default to "user"
        serde_json::Value::Object(serde_json::Map::new()),
        access_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;

    let new_refresh = sign_refresh_token(
        &signing_key,
        issuer,
        &client.client_id,
        &claims.sub,
        refresh_ttl,
    )
    .map_err(|e| OAuthError::ServerError(format!("Failed to sign refresh token: {}", e)))?;

    // Atomic rotation: mark old refresh token as replaced, insert new one
    let rotate_result = rotate_refresh_record(
        state.storage.as_ref(),
        &record,
        refresh_token_str,
        &new_refresh,
        refresh_ttl,
    )
    .await;

    match rotate_result {
        Ok(()) => {}
        Err(StorageError::TransactionConflict | StorageError::ConditionFailed(_)) => {
            let _ = log_refresh_event(
                state.storage.as_ref(),
                "refresh_token_race",
                &record,
                refresh_token_str,
                ip.as_deref(),
                Some("token already rotated"),
            )
            .await;
            return Err(OAuthError::InvalidGrant("Refresh token already used".to_string()));
        }
        Err(e) => {
            return Err(OAuthError::ServerError(format!(
                "Refresh token rotation failed: {}",
                e
            )));
        }
    }

    Ok(TokenResponse {
        access_token: new_access,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        id_token: None,
        refresh_token: Some(new_refresh),
        scope: params.scope.clone(),
    })
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

async fn rotate_refresh_record<S: StorageAdapter>(
    storage: &S,
    record: &RefreshTokenRecord,
    old_token: &str,
    new_token: &str,
    refresh_ttl: u64,
) -> Result<(), StorageError> {
    let old_key = vec!["oauth:refresh".to_string(), old_token.to_string()];
    let new_key = vec!["oauth:refresh".to_string(), new_token.to_string()];
    let now = Utc::now();
    let expiry = now + Duration::seconds(refresh_ttl as i64);

    let updated_old = RefreshTokenRecord {
        client_id: record.client_id.clone(),
        subject: record.subject.clone(),
        issued_at: record.issued_at,
        expires_at: record.expires_at,
        last_used_at: Some(now),
        replaced_by: Some(new_token.to_string()),
        revoked_at: record.revoked_at,
    };

    let old_value = serde_json::to_value(record)
        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;
    let new_old_value = serde_json::to_value(&updated_old)
        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;
    let new_record = RefreshTokenRecord {
        client_id: record.client_id.clone(),
        subject: record.subject.clone(),
        issued_at: Some(now),
        expires_at: Some(expiry),
        last_used_at: Some(now),
        replaced_by: None,
        revoked_at: None,
    };
    let new_value = serde_json::to_value(&new_record)
        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

    storage
        .transact(vec![
            TransactOperation::ConditionCheck {
                key: old_key.clone(),
                condition: TransactCondition::AttributeEquals {
                    name: "value".to_string(),
                    value: old_value,
                },
            },
            TransactOperation::Update {
                key: old_key,
                updates: new_old_value,
                condition: None,
            },
            TransactOperation::Put {
                key: new_key,
                value: new_value,
                expiry: Some(expiry),
            },
        ])
        .await
}

async fn log_refresh_event<S: StorageAdapter>(
    storage: &S,
    event_type: &str,
    record: &RefreshTokenRecord,
    token: &str,
    ip: Option<&str>,
    detail: Option<&str>,
) -> Result<(), String> {
    let mut event = AuditEvent::new(event_type);
    event.client_id = Some(record.client_id.clone());
    event.subject = Some(record.subject.clone());
    event.token_hash = Some(hash_token(token));
    event.ip = ip.map(|s| s.to_string());
    event.detail = detail.map(|s| s.to_string());

    audit::record_event(storage, event).await
}

pub(crate) async fn revoke_refresh_tokens<S: StorageAdapter>(
    storage: &S,
    subject: Option<&str>,
    client_id: Option<&str>,
) -> Result<u64, StorageError> {
    let entries = storage.scan(&["oauth:refresh"]).await?;
    let mut revoked = 0u64;
    let now = Utc::now();

    for (key, value) in entries {
        let record = serde_json::from_value::<RefreshTokenRecord>(value.clone()).ok();

        let (matches_subject, matches_client) = match (&record, subject, client_id) {
            (Some(r), Some(sub), Some(cid)) => (r.subject == sub, r.client_id == cid),
            (Some(r), Some(sub), None) => (r.subject == sub, true),
            (Some(r), None, Some(cid)) => (true, r.client_id == cid),
            (Some(_), None, None) => (false, false),
            (None, Some(sub), Some(cid)) => {
                let s = value.get("subject").and_then(|v| v.as_str()) == Some(sub);
                let c = value.get("client_id").and_then(|v| v.as_str()) == Some(cid);
                (s, c)
            }
            (None, Some(sub), None) => {
                let s = value.get("subject").and_then(|v| v.as_str()) == Some(sub);
                (s, true)
            }
            (None, None, Some(cid)) => {
                let c = value.get("client_id").and_then(|v| v.as_str()) == Some(cid);
                (true, c)
            }
            (None, None, None) => (false, false),
        };

        if !(matches_subject && matches_client) {
            continue;
        }

        let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();

        if let Some(mut r) = record {
            if r.revoked_at.is_none() {
                r.revoked_at = Some(now);
            }
            let updated = serde_json::to_value(&r)
                .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

            if let Some(expiry) = r.expires_at {
                storage.set(&key_refs, updated, Some(expiry)).await?;
            } else {
                storage.remove(&key_refs).await?;
            }
        } else {
            storage.remove(&key_refs).await?;
        }

        revoked += 1;
    }

    Ok(revoked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{environment::RuntimeAuthConfig, AppState, Config, ProviderConfig};
    use crate::config::RateLimit;
    use crate::email::NoopEmailSender;
    use crate::storage::test_support::TestStorage;
    use axum::{extract::State, Form};
    use axum::http::{HeaderMap, StatusCode};
    use std::collections::HashMap;
    use std::sync::Arc;
    use serde_json::json;

    #[tokio::test]
    async fn revoke_tokens_marks_record_revoked() {
        let storage = TestStorage::new();
        let now = Utc::now();
        let record = RefreshTokenRecord {
            client_id: "client-a".to_string(),
            subject: "user-1".to_string(),
            issued_at: Some(now),
            expires_at: Some(now + Duration::seconds(3600)),
            last_used_at: None,
            replaced_by: None,
            revoked_at: None,
        };

        storage
            .set(
                &["oauth:refresh", "token-1"],
                serde_json::to_value(&record).unwrap(),
                record.expires_at,
            )
            .await
            .unwrap();

        let revoked = revoke_refresh_tokens(&storage, Some("user-1"), Some("client-a"))
            .await
            .unwrap();
        assert_eq!(revoked, 1);

        let updated = storage
            .get(&["oauth:refresh", "token-1"])
            .await
            .unwrap()
            .unwrap();
        let updated_record: RefreshTokenRecord = serde_json::from_value(updated).unwrap();
        assert!(updated_record.revoked_at.is_some());
    }

    #[tokio::test]
    async fn revoke_tokens_respects_filters() {
        let storage = TestStorage::new();
        let now = Utc::now();

        let record_a = RefreshTokenRecord {
            client_id: "client-a".to_string(),
            subject: "user-1".to_string(),
            issued_at: Some(now),
            expires_at: Some(now + Duration::seconds(3600)),
            last_used_at: None,
            replaced_by: None,
            revoked_at: None,
        };

        let record_b = RefreshTokenRecord {
            client_id: "client-b".to_string(),
            subject: "user-2".to_string(),
            issued_at: Some(now),
            expires_at: Some(now + Duration::seconds(3600)),
            last_used_at: None,
            replaced_by: None,
            revoked_at: None,
        };

        storage
            .set(
                &["oauth:refresh", "token-a"],
                serde_json::to_value(&record_a).unwrap(),
                record_a.expires_at,
            )
            .await
            .unwrap();
        storage
            .set(
                &["oauth:refresh", "token-b"],
                serde_json::to_value(&record_b).unwrap(),
                record_b.expires_at,
            )
            .await
            .unwrap();

        let revoked = revoke_refresh_tokens(&storage, Some("user-1"), None)
            .await
            .unwrap();
        assert_eq!(revoked, 1);

        let remaining = storage
            .get(&["oauth:refresh", "token-b"])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(remaining["client_id"], json!("client-b"));
    }

    #[tokio::test]
    async fn refresh_token_reuse_logs_audit_event() {
        let storage = TestStorage::new();
        let config = Config::dev();
        let state = AppState {
            storage: Arc::new(storage),
            config: Arc::new(config),
            runtime: Arc::new(RuntimeAuthConfig::for_tests()),
            providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
            email_sender: Arc::new(NoopEmailSender::default()),
        };

        // Seed a refresh token record that was already rotated.
        let record = RefreshTokenRecord {
            client_id: "client-a".to_string(),
            subject: "user-1".to_string(),
            issued_at: Some(Utc::now()),
            expires_at: Some(Utc::now() + Duration::seconds(3600)),
            last_used_at: Some(Utc::now()),
            replaced_by: Some("new-token".to_string()),
            revoked_at: None,
        };

        state
            .storage
            .set(
                &["oauth:refresh", "old-token"],
                serde_json::to_value(&record).unwrap(),
                record.expires_at,
            )
            .await
            .unwrap();

        // Directly log the reuse event and verify audit entry exists.
        log_refresh_event(
            state.storage.as_ref(),
            "refresh_token_reuse",
            &record,
            "old-token",
            Some("127.0.0.1"),
            Some("token already rotated"),
        )
        .await
        .unwrap();

        let entries = state.storage.scan(&["audit"]).await.unwrap();
        assert!(!entries.is_empty());
    }

    #[tokio::test]
    async fn refresh_token_rotation_race_detected() {
        let storage = TestStorage::new();
        let now = Utc::now();
        let record = RefreshTokenRecord {
            client_id: "client-a".to_string(),
            subject: "user-1".to_string(),
            issued_at: Some(now),
            expires_at: Some(now + Duration::seconds(3600)),
            last_used_at: None,
            replaced_by: None,
            revoked_at: None,
        };

        storage
            .set(
                &["oauth:refresh", "old-token"],
                serde_json::to_value(&record).unwrap(),
                record.expires_at,
            )
            .await
            .unwrap();

        // Simulate concurrent rotation by updating the stored value.
        let mut updated = record.clone();
        updated.replaced_by = Some("other-token".to_string());
        updated.last_used_at = Some(Utc::now());
        storage
            .set(
                &["oauth:refresh", "old-token"],
                serde_json::to_value(&updated).unwrap(),
                updated.expires_at,
            )
            .await
            .unwrap();

        let result = rotate_refresh_record(
            &storage,
            &record,
            "old-token",
            "new-token",
            3600,
        )
        .await;

        assert!(
            matches!(result, Err(StorageError::ConditionFailed(_)) | Err(StorageError::TransactionConflict))
        );
    }

    #[tokio::test]
    async fn token_rate_limit_uses_body_client_id() {
        let storage = TestStorage::new();
        let mut config = Config::dev();
        config.rate_limit.limits.insert(
            Endpoint::Token,
            RateLimit {
                requests: 1,
                window_seconds: 60,
            },
        );

        let state = AppState {
            storage: Arc::new(storage),
            config: Arc::new(config),
            runtime: Arc::new(RuntimeAuthConfig::for_tests()),
            providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
            email_sender: Arc::new(NoopEmailSender::default()),
        };

        let params = TokenRequest {
            grant_type: "authorization_code".to_string(),
            client_id: Some("client-a".to_string()),
            client_secret: None,
            code: None,
            redirect_uri: None,
            code_verifier: None,
            refresh_token: None,
            scope: None,
        };

        let _ = handle_token(
            State(state.clone()),
            HeaderMap::new(),
            Form(params.clone()),
        )
        .await;

        let res = handle_token(
            State(state),
            HeaderMap::new(),
            Form(params),
        )
        .await
        .unwrap();

        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
