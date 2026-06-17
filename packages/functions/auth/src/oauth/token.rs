//! Token endpoint (/token).
//!
//! Handles authorization-code exchange and refresh-token rotation.

use axum::{
    extract::{Extension, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
    Form,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::audit::AuditEvent;
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
use crate::error::OAuthError;
use crate::oauth::pkce::validate_pkce;
use crate::ratelimit::middleware::trusted_source_ip_from_context;
use crate::store::rate_limits::client_source_rate_limit_identifier;
use crate::store::records::RefreshTokenRecord as StoreRefreshTokenRecord;
use crate::store::refresh::{CreateRefreshTokenInput, RefreshTokenStoreError};

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

/// Handle the token request.
pub async fn handle_token(
    State(state): State<AppState>,
    context: Option<Extension<lambda_http::request::RequestContext>>,
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

    // Pre-auth token throttling uses client plus trusted source so one caller
    // cannot exhaust the bucket for all users of a public client.
    let ip = context
        .as_ref()
        .and_then(|Extension(context)| trusted_source_ip_from_context(context));
    let identifier = client_source_rate_limit_identifier(Some(&client_id), ip.as_deref());
    if let Err(err) = state
        .store
        .check_rate_limit(&state.config.rate_limit, Endpoint::Token, &identifier)
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

async fn handle_authorization_code_grant(
    state: &AppState,
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
    let code_data = state
        .store
        .get_authorization_code(&code_digest)
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
    if !state
        .store
        .is_active_account(&subject)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
    {
        return Err(OAuthError::InvalidGrant(
            "subject account is not active".to_string(),
        ));
    }

    let code_data = state
        .store
        .delete_authorization_code_if_current(&code_digest, code_data)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| {
            OAuthError::InvalidGrant("Invalid or expired authorization code".to_string())
        })?;

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
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;
    let id_token = match build_id_token_claims(issuer, &client.client_id, &code_data, id_ttl) {
        Some(claims) => Some(
            state
                .runtime
                .signer
                .sign_id_token(&claims)
                .await
                .map_err(|e| OAuthError::ServerError(format!("Failed to sign ID token: {}", e)))?,
        ),
        None => None,
    };
    let refresh_token = if scope_contains(&code_data.scope, OFFLINE_ACCESS) {
        if !client
            .allowed_grant_types
            .contains(&GrantType::RefreshToken)
        {
            return Err(OAuthError::UnauthorizedClient(
                "client is not allowed to receive refresh tokens".to_string(),
            ));
        }
        let refresh_expires_at =
            Utc::now() + Duration::seconds(state.runtime.ttls.refresh_token_seconds as i64);
        let created = state
            .store
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
        let _ = state
            .store
            .record_audit_event_if_enabled(state.runtime.audit_log_mode, refresh_event)
            .await;

        Some(created.raw_token)
    } else {
        None
    };

    let mut event = AuditEvent::new("authorization_code_exchanged");
    event.client_id = Some(client.client_id.clone());
    event.subject = Some(code_data.subject.clone());
    let _ = state
        .store
        .record_audit_event_if_enabled(state.runtime.audit_log_mode, event)
        .await;

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        id_token,
        refresh_token,
        scope: Some(code_data.scope),
    })
}

async fn handle_target_refresh_token_grant(
    state: &AppState,
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
    let refresh_expires_at =
        Utc::now() + Duration::seconds(state.runtime.ttls.refresh_token_seconds as i64);

    let rotated = match state
        .store
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
            let _ = state
                .store
                .record_audit_event_if_enabled(state.runtime.audit_log_mode, event)
                .await;
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
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to sign access token: {}", e)))?;

    let mut event = AuditEvent::new("refresh_token_rotated");
    event.client_id = Some(client.client_id.clone());
    event.subject = Some(rotated.subject.clone());
    event.token_hash = Some(rotated.refresh_digest.clone());
    event.ip = ip;
    let _ = state
        .store
        .record_audit_event_if_enabled(state.runtime.audit_log_mode, event)
        .await;

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl,
        id_token: None,
        refresh_token: Some(rotated.raw_token),
        scope: Some(rotated.scope),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RateLimit;
    use crate::config::{environment::RuntimeAuthConfig, AppState, Config};
    use crate::email::NoopEmailSender;
    use crate::storage::test_support::TestStorage;
    use crate::store::AuthStore;
    use axum::http::{HeaderMap, StatusCode};
    use axum::{extract::State, Form};
    use std::sync::Arc;

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
            store: AuthStore::new(storage),
            config: Arc::new(config),
            runtime: Arc::new(RuntimeAuthConfig::for_tests()),
            email_sender: Arc::new(NoopEmailSender::default()),
            google_client: Arc::new(crate::providers::google::ReqwestGoogleOidcClient::new()),
            apple_client: Arc::new(crate::providers::apple::ReqwestAppleOidcClient::new()),
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
            None,
            HeaderMap::new(),
            Form(params.clone()),
        )
        .await;

        let res = handle_token(State(state), None, HeaderMap::new(), Form(params))
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
