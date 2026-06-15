//! OAuth token revocation endpoint.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Form,
};
use serde::Deserialize;

use crate::audit::{self, AuditEvent};
use crate::client::parse_basic_auth;
use crate::config::AppState;
use crate::core::clients::{ClientType, GrantType, TokenEndpointAuthMethod};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use crate::store::AuthStore;

#[derive(Debug, Deserialize, Clone)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

pub async fn handle_revoke<S: StorageAdapter>(
    State(state): State<AppState<S>>,
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
    let store = AuthStore::new(state.storage.clone());
    let _ = store
        .revoke_refresh_token_family(
            state.runtime.lookup_secret.as_bytes(),
            &params.token,
            &client.client_id,
        )
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?;

    let mut event = AuditEvent::new("refresh_token_revoked");
    event.client_id = Some(client.client_id.clone());
    event.token_hash = Some(refresh_digest);
    let _ = audit::record_event(state.storage.as_ref(), event).await;

    Ok(StatusCode::OK.into_response())
}
