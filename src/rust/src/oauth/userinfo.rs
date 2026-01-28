//! UserInfo endpoint (/userinfo).
//!
//! Returns information about the authenticated user.

use axum::{
    extract::State,
    http::HeaderMap,
    response::Json,
};

use crate::config::AppState;
use crate::error::OAuthError;
use crate::jwt::{get_all_signing_keys, verify_access_token};
use crate::storage::StorageAdapter;

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

    // Get signing keys for verification
    let keys = get_all_signing_keys(state.storage.as_ref())
        .await
        .map_err(|e| OAuthError::ServerError(e))?;

    // Determine issuer URL
    let issuer = state
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://localhost");

    // Verify the access token
    let claims = verify_access_token(token, &keys, issuer, None)
        .map_err(|e| OAuthError::InvalidGrant(e))?;

    // Verify it's an access token (mode = "access")
    if claims.mode != "access" {
        return Err(OAuthError::InvalidGrant("Not an access token".to_string()));
    }

    // Return the subject information
    Ok(Json(serde_json::json!({
        "sub": claims.sub,
        "type": claims.subject_type,
        "properties": claims.properties,
    })))
}
