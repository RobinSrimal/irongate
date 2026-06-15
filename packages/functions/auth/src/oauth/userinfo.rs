//! UserInfo endpoint (/userinfo).
//!
//! Returns information about the authenticated user.

use axum::{extract::State, http::HeaderMap, response::Json};

use crate::config::AppState;
use crate::core::scopes::EMAIL;
use crate::core::subjects::Subject;
use crate::core::tokens::scope_contains;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use crate::store::AuthStore;

/// Handle the userinfo request.
pub async fn handle_userinfo<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, OAuthError> {
    // Extract Bearer token from Authorization header
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| OAuthError::InvalidRequest("Bearer token required".to_string()))?;

    // Determine issuer URL
    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://localhost");

    // Verify the access token
    let claims = state
        .runtime
        .signer
        .verify_access_token(token, issuer, &state.runtime.access_token_audience)
        .map_err(|e| OAuthError::InvalidGrant(e))?;

    let store = AuthStore::new(state.storage.clone());
    let subject = Subject::from_persisted(claims.sub.clone());
    if !store
        .is_active_account(&subject)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
    {
        return Err(OAuthError::InvalidGrant(
            "subject account is not active".to_string(),
        ));
    }

    let mut response = serde_json::json!({
        "sub": claims.sub,
        "type": claims.subject_type,
    });

    if scope_contains(&claims.scope, EMAIL) {
        if let Some(email) = claims
            .properties
            .get("email")
            .and_then(|value| value.as_str())
        {
            response["email"] = serde_json::Value::String(email.to_string());
        }
        if let Some(email_verified) = claims
            .properties
            .get("email_verified")
            .and_then(|value| value.as_bool())
        {
            response["email_verified"] = serde_json::Value::Bool(email_verified);
        }
    }

    Ok(Json(response))
}
