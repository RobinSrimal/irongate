//! Sign in with Apple provider-start API.

use axum::{
    extract::{Extension, Form, Query, State},
    response::{IntoResponse, Redirect, Response},
};
use chrono::Utc;
use lambda_http::request::RequestContext;
use serde::Deserialize;
use serde_json::json;
use url::Url;

use crate::config::{AppState, Endpoint};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::crypto::random::generate_random_string;
use crate::error::OAuthError;
use crate::oauth::pkce::{generate_challenge, generate_verifier};
use crate::providers::apple::{
    apple_callback_uri, apple_identity_digest, build_apple_authorization_url,
    generate_apple_client_secret, validate_apple_id_token, AppleAuthorizeInput,
    AppleCodeExchangeInput, AppleIdTokenValidation,
};
use crate::ratelimit::middleware::trusted_source_ip_from_context;
use crate::store::rate_limits::provider_authorize_rate_limit_identifier;
use crate::store::records::{AuthorizationCodeRecord, ProviderStateRecord};

#[derive(Debug, Deserialize)]
pub struct AppleAuthorizeQuery {
    pub session: String,
}

#[derive(Debug, Deserialize)]
pub struct AppleCallbackForm {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub user: Option<String>,
}

pub async fn apple_authorize_handler(
    State(app): State<AppState>,
    context: Option<Extension<RequestContext>>,
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
    let ip = context
        .as_ref()
        .and_then(|Extension(context)| trusted_source_ip_from_context(context));
    let identifier =
        provider_authorize_rate_limit_identifier("apple", Some(&session_digest), ip.as_deref());
    if let Err(err) = app
        .store
        .check_rate_limit(
            &app.config.rate_limit,
            Endpoint::ProviderAuthorize,
            &identifier,
        )
        .await
    {
        return Ok(err.into_response());
    }

    let store = app.store.clone();
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

pub async fn apple_callback_handler(
    State(app): State<AppState>,
    Form(form): Form<AppleCallbackForm>,
) -> Result<Response, OAuthError> {
    let apple = app
        .runtime
        .apple
        .as_ref()
        .ok_or_else(|| OAuthError::InvalidRequest("apple provider is not configured".into()))?;
    let raw_state = form
        .state
        .as_deref()
        .ok_or_else(|| OAuthError::InvalidRequest("state is required".into()))?;

    let lookup_secret = app.runtime.lookup_secret.as_bytes();
    let provider_state_digest =
        lookup_digest(lookup_secret, LookupFamily::ProviderState, raw_state);
    let store = app.store.clone();
    let provider_state = store
        .take_provider_state(&provider_state_digest)
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?
        .ok_or_else(|| OAuthError::InvalidRequest("invalid or expired provider state".into()))?;

    if provider_state.provider != "apple" {
        return Err(OAuthError::InvalidRequest(
            "provider state is not for apple".into(),
        ));
    }

    let session = store
        .take_authorize_session(&provider_state.session_lookup_digest)
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?
        .ok_or_else(|| OAuthError::InvalidRequest("invalid or expired session".into()))?;

    if session.selected_provider.as_deref() != Some("apple") {
        return Err(OAuthError::InvalidRequest(
            "authorize session is not for apple".into(),
        ));
    }

    if let Some(error) = form.error.as_deref() {
        let redirect = client_redirect_with_params(
            &session.redirect_uri,
            &[
                ("error", error),
                ("state", session.state.as_deref().unwrap_or("")),
            ],
        )?;
        return Ok(Redirect::to(&redirect).into_response());
    }

    let code = form
        .code
        .as_deref()
        .ok_or_else(|| OAuthError::InvalidRequest("code is required".into()))?;
    let redirect_uri = apple_callback_uri(app.config.issuer_url.as_deref());
    let client_secret = generate_apple_client_secret(apple, Utc::now())
        .map_err(|_| OAuthError::ServerError("failed to generate Apple client secret".into()))?;
    let token_response = app
        .apple_client
        .exchange_code(
            apple,
            AppleCodeExchangeInput {
                code,
                redirect_uri: &redirect_uri,
                code_verifier: &provider_state.pkce_verifier,
                client_secret: &client_secret,
            },
        )
        .await
        .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;
    let jwks = app
        .apple_client
        .fetch_jwks(apple)
        .await
        .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;
    let claims = validate_apple_id_token(
        &token_response.id_token,
        &jwks,
        AppleIdTokenValidation {
            issuer: &apple.issuer,
            client_id: &apple.client_id,
            nonce: &provider_state.nonce,
            now: Utc::now(),
        },
    )
    .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;

    let identity_digest = apple_identity_digest(lookup_secret, &claims.iss, &claims.sub);
    let subject = store
        .resolve_or_create_apple_identity(
            &identity_digest,
            json!({
                "provider": "apple",
                "issuer": claims.iss,
                "email": claims.email,
                "email_verified": claims.email_verified.unwrap_or(false),
                "is_private_email": claims.is_private_email
            }),
            app.runtime.account_lifecycle.deleted_identity_reuse,
        )
        .await
        .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;

    let internal_code = generate_random_string(32);
    let code_digest = lookup_digest(
        lookup_secret,
        LookupFamily::AuthorizationCode,
        &internal_code,
    );
    let now = Utc::now();
    let expires_at = now + chrono::Duration::seconds(app.runtime.ttls.auth_code_seconds as i64);
    store
        .create_authorization_code(
            &code_digest,
            AuthorizationCodeRecord {
                client_id: session.client_id,
                redirect_uri: session.redirect_uri.clone(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                properties: json!({
                    "provider": "apple",
                    "email": claims.email,
                    "email_verified": claims.email_verified.unwrap_or(false)
                }),
                code_challenge: session.code_challenge,
                code_challenge_method: session.code_challenge_method,
                scope: session.scope,
                oidc_nonce: session.oidc_nonce,
                created_at: now,
                expires_at,
            },
        )
        .await
        .map_err(|err| OAuthError::ServerError(err.to_string()))?;

    let redirect = client_redirect_with_params(
        &session.redirect_uri,
        &[
            ("code", internal_code.as_str()),
            ("state", session.state.as_deref().unwrap_or("")),
        ],
    )?;
    Ok(Redirect::to(&redirect).into_response())
}

fn client_redirect_with_params(
    redirect_uri: &str,
    params: &[(&str, &str)],
) -> Result<String, OAuthError> {
    let mut url = Url::parse(redirect_uri)
        .map_err(|_| OAuthError::ServerError("stored redirect_uri is invalid".into()))?;
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in params {
            query.append_pair(name, value);
        }
    }
    Ok(url.to_string())
}
