//! Token management endpoints.
//!
//! Provides token revocation functionality.

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::config::AppState;
use crate::error::IrongateError;
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
    Json(request): Json<RevokeTokensRequest>,
) -> Result<Json<serde_json::Value>, IrongateError> {
    if request.subject.is_none() && request.client_id.is_none() {
        return Err(crate::error::OAuthError::InvalidRequest(
            "At least one of 'subject' or 'client_id' is required".to_string(),
        )
        .into());
    }

    // Scan all refresh tokens
    let entries = state.storage.scan(&["oauth:refresh"]).await?;

    let mut revoked = 0u64;
    for (key, value) in entries {
        let matches = match (&request.subject, &request.client_id) {
            (Some(sub), Some(cid)) => {
                value.get("subject").and_then(|v| v.as_str()) == Some(sub)
                    && value.get("client_id").and_then(|v| v.as_str()) == Some(cid)
            }
            (Some(sub), None) => {
                value.get("subject").and_then(|v| v.as_str()) == Some(sub)
            }
            (None, Some(cid)) => {
                value.get("client_id").and_then(|v| v.as_str()) == Some(cid)
            }
            (None, None) => false,
        };

        if matches {
            let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
            let _ = state.storage.remove(&key_refs).await;
            revoked += 1;
        }
    }

    Ok(Json(serde_json::json!({
        "revoked": revoked,
    })))
}
