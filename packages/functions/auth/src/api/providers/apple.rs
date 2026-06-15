//! Sign in with Apple provider-start API.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use chrono::Utc;
use serde::Deserialize;

use crate::config::AppState;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::crypto::random::generate_random_string;
use crate::error::OAuthError;
use crate::oauth::pkce::{generate_challenge, generate_verifier};
use crate::providers::apple::{
    apple_callback_uri, build_apple_authorization_url, AppleAuthorizeInput,
};
use crate::storage::StorageAdapter;
use crate::store::records::ProviderStateRecord;
use crate::store::AuthStore;

#[derive(Debug, Deserialize)]
pub struct AppleAuthorizeQuery {
    pub session: String,
}

pub async fn apple_authorize_handler<S: StorageAdapter>(
    State(app): State<AppState<S>>,
    Query(query): Query<AppleAuthorizeQuery>,
) -> Result<Response, OAuthError> {
    let apple = app
        .runtime
        .apple
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("apple provider is not configured".into()))?;

    let lookup_secret = app.runtime.lookup_secret.as_bytes();
    let session_digest = lookup_digest(
        lookup_secret,
        LookupFamily::AuthorizeSession,
        &query.session,
    );
    let store = AuthStore::new(app.storage.clone());
    let session = store
        .get_authorize_session(&session_digest)
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?
        .ok_or_else(|| OAuthError::InvalidRequest("invalid or expired session".into()))?;

    if session.selected_provider.as_deref() != Some("apple") {
        return Err(OAuthError::InvalidRequest(
            "authorize session is not for apple".into(),
        ));
    }

    let raw_state = generate_random_string(32);
    let nonce = generate_random_string(32);
    let pkce_verifier = generate_verifier();
    let pkce_challenge = generate_challenge(&pkce_verifier);
    let state_digest = lookup_digest(lookup_secret, LookupFamily::ProviderState, &raw_state);
    let now = Utc::now();
    let expires_at =
        now + chrono::Duration::seconds(app.runtime.ttls.provider_state_seconds as i64);

    store
        .create_provider_state(
            &state_digest,
            ProviderStateRecord {
                session_lookup_digest: session_digest,
                provider: "apple".to_string(),
                pkce_verifier,
                nonce: nonce.clone(),
                created_at: now,
                expires_at,
            },
        )
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?;

    let redirect_uri = apple_callback_uri(app.config.issuer_url.as_deref());
    let url = build_apple_authorization_url(AppleAuthorizeInput {
        config: apple,
        redirect_uri: &redirect_uri,
        state: &raw_state,
        nonce: &nonce,
        pkce_challenge: &pkce_challenge,
    });

    Ok(Redirect::to(&url).into_response())
}
