//! Authorization endpoint (/authorize).
//!
//! Handles the authorization request and redirects to provider.

use axum::{
    extract::{Query, State},
    response::Redirect,
};
use serde::Deserialize;

use crate::client::validate_authorize_request;
use crate::config::AppState;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;

/// Authorization request query parameters
#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    /// Response type (required): "code" or "token"
    pub response_type: String,
    /// Client ID (required)
    pub client_id: String,
    /// Redirect URI (required)
    pub redirect_uri: String,
    /// State parameter (required - now mandatory for security)
    pub state: String,
    /// Scope (optional)
    pub scope: Option<String>,
    /// Provider to use directly (optional)
    pub provider: Option<String>,
    /// Audience (optional)
    pub audience: Option<String>,
    /// PKCE code challenge (required by default)
    pub code_challenge: Option<String>,
    /// PKCE code challenge method (default: S256)
    pub code_challenge_method: Option<String>,
}

/// Handle the authorization request.
pub async fn handle_authorize<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Query(params): Query<AuthorizeRequest>,
) -> Result<Redirect, OAuthError> {
    // Validate client and request
    let client = validate_authorize_request(
        state.storage.as_ref(),
        &params.client_id,
        &params.redirect_uri,
        &params.response_type,
        params.code_challenge.as_deref(),
    )
    .await?;

    // Validate code_challenge_method if provided
    if let Some(method) = &params.code_challenge_method {
        if method != "S256" {
            return Err(OAuthError::InvalidRequest(
                "Only S256 code_challenge_method is supported".to_string(),
            ));
        }
    }

    // TODO: Store authorization state in encrypted cookie

    // If provider is specified, redirect directly
    if let Some(provider) = &params.provider {
        return Ok(Redirect::to(&format!("/{}/authorize?{}", provider, "TODO")));
    }

    // Otherwise, show provider selection UI
    todo!("Implement provider selection UI redirect")
}
