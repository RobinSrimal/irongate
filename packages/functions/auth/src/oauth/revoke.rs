//! OAuth token revocation endpoint.

use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Form,
};
use lambda_http::request::RequestContext;
use serde::Deserialize;

use crate::audit::AuditEvent;
use crate::client::parse_basic_auth;
use crate::config::{AppState, Endpoint};
use crate::core::clients::{ClientType, GrantType, TokenEndpointAuthMethod};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::error::OAuthError;
use crate::ratelimit::middleware::trusted_source_ip_from_context;
use crate::store::rate_limits::client_source_rate_limit_identifier;
use crate::store::refresh::RevokeRefreshTokenOutcome;

#[derive(Debug, Deserialize, Clone)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

pub async fn handle_revoke(
    State(state): State<AppState>,
    context: Option<Extension<RequestContext>>,
    headers: HeaderMap,
    Form(params): Form<RevokeRequest>,
) -> Result<Response, OAuthError> {
    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());

    let client_id = if let Some(id) = params.client_id.as_deref() {
        id.to_string()
    } else if let Some(header) = auth_header {
        let (id, _) = parse_basic_auth(Some(header))?;
        id
    } else {
        return Err(OAuthError::InvalidRequest("client_id required".to_string()));
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
        .validate_token_request(&client_id, GrantType::RefreshToken, provided_secret)
        .map_err(OAuthError::from)?;

    let ip = context
        .as_ref()
        .and_then(|Extension(context)| trusted_source_ip_from_context(context));
    let identifier = client_source_rate_limit_identifier(Some(&client.client_id), ip.as_deref());
    if let Err(err) = state
        .store
        .check_rate_limit(&state.config.rate_limit, Endpoint::OAuthRevoke, &identifier)
        .await
    {
        return Ok(err.into_response());
    }

    if params
        .token_type_hint
        .as_deref()
        .map_or(false, |hint| hint != "refresh_token")
    {
        return Ok(StatusCode::OK.into_response());
    }

    let refresh_digest = lookup_digest(
        state.runtime.lookup_secret.as_bytes(),
        LookupFamily::RefreshToken,
        &params.token,
    );
    let outcome = state
        .store
        .revoke_refresh_token_family(
            state.runtime.lookup_secret.as_bytes(),
            &params.token,
            &client.client_id,
        )
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?;

    if matches!(
        outcome,
        RevokeRefreshTokenOutcome::Revoked | RevokeRefreshTokenOutcome::AlreadyRevoked
    ) {
        let mut event = AuditEvent::new("refresh_token_revoked");
        event.client_id = Some(client.client_id.clone());
        event.token_hash = Some(refresh_digest);
        let _ = state
            .store
            .record_audit_event_if_enabled(state.runtime.audit_log_mode, event)
            .await;
    }

    Ok(StatusCode::OK.into_response())
}
