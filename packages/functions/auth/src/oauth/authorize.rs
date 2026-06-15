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

use crate::config::AppState;
use crate::core::scopes::OPENID;
use crate::crypto::encrypt::SecureCookie;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::crypto::random::generate_random_string;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use crate::store::records::AuthorizeSessionRecord;
use crate::store::AuthStore;

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
    pub nonce: Option<String>,
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
    let client = app
        .runtime
        .client_registry
        .validate_authorize_request(
            &params.client_id,
            &params.redirect_uri,
            &params.response_type,
            params.code_challenge.as_deref(),
        )
        .map_err(OAuthError::from)?;

    // Validate code_challenge_method if provided
    if let Some(method) = &params.code_challenge_method {
        if method != "S256" {
            return Err(OAuthError::InvalidRequest(
                "Only S256 code_challenge_method is supported".to_string(),
            ));
        }
    }

    let selected_provider = params
        .provider
        .as_deref()
        .ok_or_else(|| OAuthError::InvalidRequest("provider is required".to_string()))?;
    let redirect_provider = match selected_provider {
        "password" => "password",
        "google" if app.runtime.google.is_some() => "google",
        "google" => {
            return Err(OAuthError::InvalidRequest(
                "google provider is not configured".to_string(),
            ));
        }
        "apple" if app.runtime.apple.is_some() => "apple",
        "apple" => {
            return Err(OAuthError::InvalidRequest(
                "apple provider is not configured".to_string(),
            ));
        }
        _ => {
            return Err(OAuthError::InvalidRequest(
                "provider is not supported by this auth core yet".to_string(),
            ));
        }
    };

    let scope = normalize_authorize_scope(params.scope.as_deref(), &client.allowed_scopes)?;
    let oidc_nonce = scope
        .split_whitespace()
        .any(|scope| scope == OPENID)
        .then(|| params.nonce.clone())
        .flatten();

    // Generate internal session key and store session in DynamoDB.
    let session_key = generate_random_string(32);
    let expires_at = chrono::Utc::now()
        + chrono::Duration::seconds(app.runtime.ttls.authorize_session_seconds as i64);
    let session = AuthorizeSessionRecord {
        client_id: params.client_id.clone(),
        redirect_uri: params.redirect_uri.clone(),
        state: Some(params.state.clone()),
        scope,
        oidc_nonce,
        code_challenge: params.code_challenge.clone(),
        code_challenge_method: params.code_challenge_method.clone(),
        selected_provider: Some(selected_provider.to_string()),
        created_at: chrono::Utc::now(),
        expires_at,
    };
    let session_digest = lookup_digest(
        app.runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        &session_key,
    );
    let store = AuthStore::new(app.storage.clone());
    store
        .create_authorize_session(&session_digest, session)
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to store session: {}", e)))?;

    // Set session key in a secure cookie
    let cookie = SecureCookie::new("irongate_session", &session_key).max_age(600);

    // Determine redirect target
    let redirect_url = match redirect_provider {
        "password" => format!("/password/login?session={session_key}"),
        "google" => format!("/google/authorize?session={session_key}"),
        "apple" => format!("/apple/authorize?session={session_key}"),
        _ => unreachable!("unsupported provider was already rejected"),
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

fn normalize_authorize_scope(
    requested_scope: Option<&str>,
    allowed_scopes: &[String],
) -> Result<String, OAuthError> {
    let raw = requested_scope.unwrap_or(OPENID);
    let mut normalized = Vec::new();
    for scope in raw.split_whitespace() {
        if scope.is_empty() || normalized.iter().any(|existing| existing == scope) {
            continue;
        }
        if !allowed_scopes.iter().any(|allowed| allowed == scope) {
            return Err(OAuthError::InvalidScope(format!(
                "scope `{scope}` is not allowed for this client"
            )));
        }
        normalized.push(scope.to_string());
    }

    if normalized.is_empty() {
        return Err(OAuthError::InvalidScope(
            "at least one scope is required".to_string(),
        ));
    }

    Ok(normalized.join(" "))
}
