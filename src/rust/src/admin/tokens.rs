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
    todo!("Implement token revocation")
}
