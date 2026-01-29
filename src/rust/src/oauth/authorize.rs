//! Authorization endpoint (/authorize).
//!
//! Validates the client, stores authorization session state in DynamoDB,
//! and redirects to the identity provider or provider selection UI.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use http::header::SET_COOKIE;
use serde::{Deserialize, Serialize};

use crate::client::validate_authorize_request;
use crate::config::AppState;
use crate::crypto::encrypt::SecureCookie;
use crate::crypto::random::generate_random_string;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;

/// Authorization request query parameters
#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    pub scope: Option<String>,
    pub provider: Option<String>,
    pub audience: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

/// Authorization session stored in DynamoDB
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorizeSession {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub state: String,
    pub scope: Option<String>,
    pub audience: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

/// Handle the authorization request.
pub async fn handle_authorize<S: StorageAdapter>(
    State(app): State<AppState<S>>,
    Query(params): Query<AuthorizeRequest>,
) -> Result<Response, OAuthError> {
    // Validate client and request
    let _client = validate_authorize_request(
        app.storage.as_ref(),
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

    // Generate internal session key and store session in DynamoDB
    let session_key = generate_random_string(32);
    let session = AuthorizeSession {
        client_id: params.client_id.clone(),
        redirect_uri: params.redirect_uri.clone(),
        response_type: params.response_type.clone(),
        state: params.state.clone(),
        scope: params.scope.clone(),
        audience: params.audience.clone(),
        code_challenge: params.code_challenge.clone(),
        code_challenge_method: params.code_challenge_method.clone(),
    };

    let session_value = serde_json::to_value(&session)
        .map_err(|e| OAuthError::ServerError(format!("Failed to serialize session: {}", e)))?;

    // Store session with 10-minute TTL
    let expiry = chrono::Utc::now() + chrono::Duration::seconds(600);
    app.storage
        .set(
            &["oauth:session", &session_key],
            session_value,
            Some(expiry),
        )
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to store session: {}", e)))?;

    // Set session key in a secure cookie
    let cookie = SecureCookie::new("irongate_session", &session_key).max_age(600);

    // Determine redirect target
    let redirect_url = if let Some(provider) = &params.provider {
        format!(
            "/provider/{}/authorize?session={}",
            provider, session_key
        )
    } else {
        format!("/ui/select?session={}", session_key)
    };

    let mut response = Redirect::to(&redirect_url).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        cookie
            .to_header_value()
            .parse()
            .map_err(|_| OAuthError::ServerError("Invalid cookie header".to_string()))?,
    );

    Ok(response)
}
