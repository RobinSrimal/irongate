//! Token management endpoints.
//!
//! Provides token revocation functionality.

use axum::{extract::State, Extension, Json};
use serde::Deserialize;

use crate::admin::auth::{AdminContext, require_permission};
use crate::config::AppState;
use crate::error::IrongateError;
use crate::oauth::token::revoke_refresh_tokens;
use crate::storage::StorageAdapter;

/// Request to revoke tokens
#[derive(Debug, Deserialize)]
pub struct RevokeTokensRequest {
    /// Subject ID to revoke tokens for
    pub subject: Option<String>,
    /// Client ID to revoke tokens for
    pub client_id: Option<String>,
}

/// Revoke tokens for a subject or client
pub async fn revoke_tokens<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Extension(ctx): Extension<AdminContext>,
    Json(request): Json<RevokeTokensRequest>,
) -> Result<Json<serde_json::Value>, IrongateError> {
    require_permission(&ctx, "tokens:revoke")?;
    if request.subject.is_none() && request.client_id.is_none() {
        return Err(crate::error::OAuthError::InvalidRequest(
            "At least one of 'subject' or 'client_id' is required".to_string(),
        )
        .into());
    }

    let revoked = revoke_refresh_tokens(
        state.storage.as_ref(),
        request.subject.as_deref(),
        request.client_id.as_deref(),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "revoked": revoked,
    })))
}
