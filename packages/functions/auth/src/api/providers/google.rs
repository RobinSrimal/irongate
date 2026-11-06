//! Google OIDC provider-start API.

use axum::{
    extract::rejection::QueryRejection,
    extract::{Extension, Query, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
};
use chrono::Utc;
use lambda_http::request::RequestContext;
use serde::Deserialize;
use serde_json::json;
use url::Url;

use super::{browser_binding, issuance::map_storage_error};
use crate::config::{AppState, Endpoint};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::crypto::random::generate_random_string;
use crate::error::OAuthError;
use crate::oauth::pkce::{generate_challenge, generate_verifier};
use crate::providers::google::{
    build_google_authorization_url, google_callback_uri, google_identity_digest,
    validate_google_id_token, GoogleAuthorizeInput, GoogleCodeExchangeInput,
    GoogleIdTokenValidation, GoogleOidcError,
};
use crate::providers::verified_email::ProviderVerifiedEmail;
use crate::ratelimit::middleware::trusted_source_ip_from_context;
use crate::store::provider_issuance::ProviderIdentityInput;
use crate::store::provider_states::ProviderStateProof;
use crate::store::rate_limits::{RateLimitIdentity, RateLimitSubject};
use crate::store::records::{ProviderStateBinding, ProviderStateRecord};
use crate::store::IdentityProvider;

#[derive(Debug, Deserialize)]
pub struct GoogleAuthorizeQuery {
    pub session: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn google_authorize_handler(
    State(app): State<AppState>,
    context: Option<Extension<RequestContext>>,
    query: Result<Query<GoogleAuthorizeQuery>, QueryRejection>,
) -> Response {
    let result = match query {
        Ok(Query(query)) => authorize(&app, context, query).await,
        Err(_) => Err(browser_binding::invalid()),
    };
    browser_binding::protect(result)
}

async fn authorize(
    app: &AppState,
    context: Option<Extension<RequestContext>>,
    query: GoogleAuthorizeQuery,
) -> Result<Response, OAuthError> {
    let google =
        app.runtime.google.as_ref().ok_or_else(|| {
            OAuthError::InvalidRequest("google provider is not configured".into())
        })?;

    let lookup_secret = app.runtime.lookup_secret.as_bytes();
    let session_digest = lookup_digest(
        lookup_secret,
        LookupFamily::AuthorizeSession,
        &query.session,
    );
    let ip = context
        .as_ref()
        .and_then(|Extension(context)| trusted_source_ip_from_context(context));
    if let Err(err) = app
        .store
        .check_rate_limit(
            &app.config.rate_limit,
            Endpoint::ProviderAuthorize,
            RateLimitIdentity {
                secret: lookup_secret,
                source: ip.as_deref(),
                subject: RateLimitSubject::Provider {
                    provider: "google",
                    session_digest: &session_digest,
                },
            },
        )
        .await
    {
        return Ok(err.into_response());
    }

    let store = app.store.clone();
    let session = store
        .get_authorize_session(&session_digest)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| OAuthError::InvalidRequest("invalid or expired session".into()))?;

    if session.selected_provider.as_deref() != Some("google") {
        return Err(OAuthError::InvalidRequest(
            "authorize session is not for google".into(),
        ));
    }

    let raw_state = generate_random_string(32);
    let browser = generate_random_string(32);
    let nonce = generate_random_string(32);
    let pkce_verifier = generate_verifier();
    let pkce_challenge = generate_challenge(&pkce_verifier);
    let state_digest = lookup_digest(lookup_secret, LookupFamily::ProviderState, &raw_state);
    let now = Utc::now();
    if now >= session.expires_at {
        return Err(browser_binding::invalid());
    }
    let duration = i64::try_from(app.runtime.ttls.provider_state_seconds)
        .ok()
        .filter(|seconds| *seconds > 0)
        .and_then(chrono::Duration::try_seconds)
        .ok_or_else(|| OAuthError::ServerError("invalid provider state lifetime".into()))?;
    let expires_at = now
        .checked_add_signed(duration)
        .ok_or_else(|| OAuthError::ServerError("invalid provider state lifetime".into()))?
        .min(session.expires_at);

    store
        .create_provider_state(
            &state_digest,
            ProviderStateRecord {
                session_lookup_digest: session_digest,
                binding: ProviderStateBinding::Google {
                    browser_digest: lookup_digest(
                        lookup_secret,
                        LookupFamily::SignInBrowser,
                        &browser,
                    ),
                },
                pkce_verifier,
                nonce: nonce.clone(),
                created_at: now,
                expires_at,
            },
        )
        .await
        .map_err(map_storage_error)?;

    let redirect_uri = google_callback_uri(app.config.issuer_url.as_deref());
    let url = build_google_authorization_url(GoogleAuthorizeInput {
        config: google,
        redirect_uri: &redirect_uri,
        state: &raw_state,
        nonce: &nonce,
        pkce_challenge: &pkce_challenge,
    });

    let max_age = (expires_at - Utc::now()).num_seconds();
    if max_age <= 0 {
        return Err(browser_binding::invalid());
    }
    let mut response = Redirect::to(&url).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        browser_binding::header(&state_digest, &browser, max_age),
    );
    Ok(response)
}

pub async fn google_callback_handler(
    State(app): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<GoogleCallbackQuery>, QueryRejection>,
) -> Response {
    let mut consumed = None;
    let result = match query {
        Ok(Query(query)) => callback(&app, &headers, query, &mut consumed).await,
        Err(_) => Err(browser_binding::invalid()),
    };
    let mut response = browser_binding::protect(result);
    if let Some(digest) = consumed {
        response
            .headers_mut()
            .append(SET_COOKIE, browser_binding::header(&digest, "", 0));
    }
    response
}

async fn callback(
    app: &AppState,
    headers: &HeaderMap,
    query: GoogleCallbackQuery,
    consumed: &mut Option<String>,
) -> Result<Response, OAuthError> {
    let google =
        app.runtime.google.as_ref().ok_or_else(|| {
            OAuthError::InvalidRequest("google provider is not configured".into())
        })?;
    let raw_state = query
        .state
        .as_deref()
        .filter(|state| browser_binding::valid_handle(state))
        .ok_or_else(browser_binding::invalid)?;

    let lookup_secret = app.runtime.lookup_secret.as_bytes();
    let provider_state_digest =
        lookup_digest(lookup_secret, LookupFamily::ProviderState, raw_state);
    let browser = browser_binding::read(headers, &provider_state_digest)?;
    let browser_digest = lookup_digest(lookup_secret, LookupFamily::SignInBrowser, &browser);
    let store = app.store.clone();
    let provider_state = store
        .take_provider_state(
            &provider_state_digest,
            ProviderStateProof::GoogleBrowser(&browser_digest),
        )
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| OAuthError::InvalidRequest("invalid or expired provider state".into()))?;

    *consumed = Some(provider_state_digest);

    let session = store
        .take_authorize_session(&provider_state.session_lookup_digest)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| OAuthError::InvalidRequest("invalid or expired session".into()))?;

    if session.selected_provider.as_deref() != Some("google") {
        return Err(OAuthError::InvalidRequest(
            "authorize session is not for google".into(),
        ));
    }

    if let Some(error) = query.error.as_deref() {
        let redirect = client_redirect_with_params(
            &session.redirect_uri,
            &[
                ("error", error),
                ("state", session.state.as_deref().unwrap_or("")),
            ],
        )?;
        return Ok(Redirect::to(&redirect).into_response());
    }

    let code = query
        .code
        .as_deref()
        .ok_or_else(|| OAuthError::InvalidRequest("code is required".into()))?;
    let redirect_uri = google_callback_uri(app.config.issuer_url.as_deref());
    let token_response = app
        .google_client
        .exchange_code(
            google,
            GoogleCodeExchangeInput {
                code,
                redirect_uri: &redirect_uri,
                code_verifier: &provider_state.pkce_verifier,
            },
        )
        .await
        .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;
    let token_validation = GoogleIdTokenValidation {
        issuer: &google.issuer,
        client_id: &google.client_id,
        nonce: &provider_state.nonce,
        now: Utc::now(),
    };
    let jwks = app
        .google_client
        .fetch_jwks(google)
        .await
        .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;
    let claims = match validate_google_id_token(&token_response.id_token, &jwks, token_validation) {
        Ok(claims) => claims,
        Err(GoogleOidcError::MissingKey) => {
            let fresh_jwks = app
                .google_client
                .refresh_jwks(google)
                .await
                .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?;
            validate_google_id_token(&token_response.id_token, &fresh_jwks, token_validation)
                .map_err(|err| OAuthError::InvalidGrant(err.to_string()))?
        }
        Err(err) => return Err(OAuthError::InvalidGrant(err.to_string())),
    };

    let identity_digest = google_identity_digest(lookup_secret, &claims.iss, &claims.sub);
    let prepared = store
        .prepare_provider_issuance(
            ProviderIdentityInput {
                provider: IdentityProvider::Google,
                verified_email: ProviderVerifiedEmail::from_google_claims(&claims, lookup_secret),
                identity_digest: &identity_digest,
                properties: json!({
                    "provider": "google",
                    "issuer": claims.iss,
                    "email": claims.email,
                    "email_verified": claims.email_verified.unwrap_or(false),
                    "name": claims.name,
                    "picture": claims.picture
                }),
            },
            &session,
            app.runtime.ttls.auth_code_seconds,
            app.runtime.account_lifecycle.deleted_identity_reuse,
        )
        .await
        .map_err(map_storage_error)?;

    let internal_code = generate_random_string(32);
    let code_digest = lookup_digest(
        lookup_secret,
        LookupFamily::AuthorizationCode,
        &internal_code,
    );
    store
        .finish_provider_issuance(prepared, &code_digest)
        .await
        .map_err(map_storage_error)?;

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
