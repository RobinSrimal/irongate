//! HTTP route definitions for Irongate OAuth 2.0 server.
//!
//! Uses Axum for routing with Lambda integration.

use axum::{
    extract::{Path, Query, Request, State},
    http::HeaderMap,
    middleware,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json,
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use url::form_urlencoded;

use crate::config::{AppState, Endpoint, ProviderConfig};
use crate::crypto::random::generate_random_string;
use crate::error::OAuthError;
use crate::flows::password::{
    register_password_user, verify_password_email, PasswordRegistrationError,
    PasswordRegistrationInput, PasswordRegistrationStatus, PasswordVerificationError,
    PasswordVerificationInput, PasswordVerificationStatus,
};
use crate::oauth;
use crate::provider::oauth2::{
    self as oauth2_provider, ProviderAuthorizeQuery, ProviderCallbackQuery, ProviderFlowState,
};
use crate::provider::traits::SubjectInfo;
use crate::storage::StorageAdapter;
use crate::store::AuthStore;
use crate::subject::Subject;
use crate::ui;

/// Create the main Axum router with all routes
pub fn create_router<S: StorageAdapter + Clone + 'static>(state: AppState<S>) -> Router {
    let rate_limit_authorize = {
        let app = state.clone();
        middleware::from_fn(move |req: Request, next: Next| {
            let app = app.clone();
            async move {
                let client_id = extract_client_id_from_request(&req);
                let ip = crate::ratelimit::middleware::extract_client_ip(
                    req.headers(),
                    &app.config.proxy,
                );
                let identifier = crate::ratelimit::middleware::get_rate_limit_identifier(
                    client_id.as_deref(),
                    ip.as_deref(),
                );

                match crate::ratelimit::middleware::check_rate_limit(
                    app.storage.as_ref(),
                    &app.config.rate_limit,
                    Endpoint::Authorize,
                    &identifier,
                )
                .await
                {
                    Ok(()) => next.run(req).await,
                    Err(err) => err.into_response(),
                }
            }
        })
    };

    let dev_mode = state.config.dev_mode;

    let cors_layer = if dev_mode {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
    };

    Router::new()
        // Well-known endpoints (public)
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth::well_known::oauth_authorization_server::<S>),
        )
        .route(
            "/.well-known/openid-configuration",
            get(oauth::well_known::openid_configuration::<S>),
        )
        .route(
            "/.well-known/jwks.json",
            get(oauth::well_known::jwks::<S>),
        )
        // OAuth endpoints
        .route(
            "/authorize",
            get(oauth::authorize::handle_authorize::<S>).route_layer(rate_limit_authorize),
        )
        .route("/token", post(oauth::token::handle_token::<S>))
        .route("/userinfo", get(oauth::userinfo::handle_userinfo::<S>))
        .route("/password/register", post(password_register_handler::<S>))
        .route("/password/verify", post(password_verify_handler::<S>))
        // Provider routes
        .route("/:provider/authorize", get(provider_authorize_handler::<S>))
        .route("/:provider/callback", get(provider_callback_get_handler::<S>))
        .route("/:provider/callback", post(provider_callback_post_handler::<S>))
        // CORS must be outermost (added last = wraps everything)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer)
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct PasswordRegisterRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct PasswordRegisterResponse {
    status: PasswordRegistrationStatus,
}

#[derive(Debug, Deserialize)]
struct PasswordVerifyRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct PasswordVerifyResponse {
    status: PasswordVerificationStatus,
    subject: String,
}

async fn password_register_handler<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    Json(payload): Json<PasswordRegisterRequest>,
) -> Result<Json<PasswordRegisterResponse>, OAuthError> {
    let store = AuthStore::new(app.storage.clone());
    let outcome = register_password_user(
        &store,
        &app.runtime,
        app.email_sender.as_ref(),
        PasswordRegistrationInput {
            email: &payload.email,
            password: &payload.password,
        },
    )
    .await
    .map_err(map_password_registration_error)?;

    Ok(Json(PasswordRegisterResponse {
        status: outcome.status,
    }))
}

async fn password_verify_handler<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    Json(payload): Json<PasswordVerifyRequest>,
) -> Result<Json<PasswordVerifyResponse>, OAuthError> {
    let store = AuthStore::new(app.storage.clone());
    let outcome = verify_password_email(
        &store,
        &app.runtime,
        PasswordVerificationInput {
            token: &payload.token,
        },
    )
    .await
    .map_err(map_password_verification_error)?;

    Ok(Json(PasswordVerifyResponse {
        status: outcome.status,
        subject: outcome.subject,
    }))
}

fn map_password_registration_error(err: PasswordRegistrationError) -> OAuthError {
    match err {
        PasswordRegistrationError::Password(_) => {
            OAuthError::InvalidRequest("invalid registration request".to_string())
        }
        PasswordRegistrationError::EmailAlreadyRegistered => {
            OAuthError::InvalidRequest("email is already registered".to_string())
        }
        PasswordRegistrationError::Storage(storage) => {
            OAuthError::ServerError(storage.to_string())
        }
        PasswordRegistrationError::EmailDelivery(_) => {
            OAuthError::TemporarilyUnavailable("email delivery failed".to_string())
        }
    }
}

fn map_password_verification_error(err: PasswordVerificationError) -> OAuthError {
    match err {
        PasswordVerificationError::InvalidVerificationToken => {
            OAuthError::InvalidGrant("verification token is invalid or expired".to_string())
        }
        PasswordVerificationError::PasswordUserNotFound => {
            OAuthError::InvalidGrant("verification token is invalid or expired".to_string())
        }
        PasswordVerificationError::Storage(storage) => {
            OAuthError::ServerError(storage.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Provider authorize handler
// ---------------------------------------------------------------------------

/// Handle GET /:provider/authorize?session={key}
///
/// Dispatches to the appropriate provider flow:
/// - OAuth2/OIDC: Build external authorization URL and redirect
/// - Password: Show login form
/// - Code: Show code request form
async fn provider_authorize_handler<S: StorageAdapter>(
    State(app): State<AppState<S>>,
    Path(provider_name): Path<String>,
    Query(query): Query<ProviderAuthorizeQuery>,
) -> Result<Response, OAuthError> {
    // Look up provider
    let provider_config = app
        .providers
        .get(&provider_name)
        .ok_or_else(|| OAuthError::InvalidRequest(format!("Unknown provider: {}", provider_name)))?;

    // Verify the session exists
    let _session = app
        .storage
        .get(&["oauth:session", &query.session])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidRequest("Invalid or expired session".to_string()))?;

    match provider_config {
        ProviderConfig::OAuth2(config) => {
            let state_key = generate_random_string(32);
            let pkce_verifier = if config.pkce {
                Some(crate::oauth::pkce::generate_verifier())
            } else {
                None
            };
            let pkce_challenge = pkce_verifier
                .as_deref()
                .map(crate::oauth::pkce::generate_challenge);

            // Store provider flow state
            let flow_state = ProviderFlowState {
                session_key: query.session.clone(),
                pkce_verifier,
            };
            let flow_value = serde_json::to_value(&flow_state)
                .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;
            let expiry = chrono::Utc::now() + chrono::Duration::seconds(600);
            app.storage
                .set(&["provider:state", &state_key], flow_value, Some(expiry))
                .await
                .map_err(|e| OAuthError::ServerError(e.to_string()))?;

            let issuer_url = app
                .config
                .issuer_url
                .as_deref()
                .unwrap_or("https://localhost");
            let callback_uri = format!("{}/{}/callback", issuer_url, provider_name);

            let url = oauth2_provider::build_authorization_url(
                config,
                &state_key,
                &callback_uri,
                pkce_challenge.as_deref(),
            );

            Ok(Redirect::to(&url).into_response())
        }
        ProviderConfig::Oidc(config) => {
            let state_key = generate_random_string(32);
            let pkce_verifier = if config.oauth2.pkce {
                Some(crate::oauth::pkce::generate_verifier())
            } else {
                None
            };
            let pkce_challenge = pkce_verifier
                .as_deref()
                .map(crate::oauth::pkce::generate_challenge);

            let flow_state = ProviderFlowState {
                session_key: query.session.clone(),
                pkce_verifier,
            };
            let flow_value = serde_json::to_value(&flow_state)
                .map_err(|e| OAuthError::ServerError(format!("Serialize error: {}", e)))?;
            let expiry = chrono::Utc::now() + chrono::Duration::seconds(600);
            app.storage
                .set(&["provider:state", &state_key], flow_value, Some(expiry))
                .await
                .map_err(|e| OAuthError::ServerError(e.to_string()))?;

            let issuer_url = app
                .config
                .issuer_url
                .as_deref()
                .unwrap_or("https://localhost");
            let callback_uri = format!("{}/{}/callback", issuer_url, provider_name);

            let url = oauth2_provider::build_authorization_url(
                &config.oauth2,
                &state_key,
                &callback_uri,
                pkce_challenge.as_deref(),
            );

            Ok(Redirect::to(&url).into_response())
        }
        ProviderConfig::Password(_) => {
            let html = ui::password::render_password_form(
                ui::password::PasswordFormMode::Login,
                None,
                Some(&format!("/{}/callback", provider_name)),
                Some(&query.session),
            );
            Ok(Html(html).into_response())
        }
        ProviderConfig::Code(_) => {
            let html = ui::code::render_code_request_form(None);
            Ok(Html(html).into_response())
        }
    }
}

// ---------------------------------------------------------------------------
// Provider callback handlers
// ---------------------------------------------------------------------------

/// Handle GET /:provider/callback (OAuth2/OIDC code exchange)
async fn provider_callback_get_handler<S: StorageAdapter>(
    State(app): State<AppState<S>>,
    Path(provider_name): Path<String>,
    Query(query): Query<ProviderCallbackQuery>,
) -> Result<Response, OAuthError> {
    let provider_config = app
        .providers
        .get(&provider_name)
        .ok_or_else(|| OAuthError::InvalidRequest(format!("Unknown provider: {}", provider_name)))?;

    // Load provider flow state
    let flow_value = app
        .storage
        .get(&["provider:state", &query.state])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidGrant("Invalid or expired state".to_string()))?;

    // Delete state (single use)
    let _ = app
        .storage
        .remove(&["provider:state", &query.state])
        .await;

    let flow_state: ProviderFlowState = serde_json::from_value(flow_value)
        .map_err(|e| OAuthError::ServerError(format!("Corrupt flow state: {}", e)))?;

    let issuer_url = app
        .config
        .issuer_url
        .as_deref()
        .unwrap_or("https://localhost");
    let callback_uri = format!("{}/{}/callback", issuer_url, provider_name);

    // Exchange code for tokens and get subject info
    let subject_info = match provider_config {
        ProviderConfig::OAuth2(config) => {
            let tokens =
                oauth2_provider::exchange_code(config, &query.code, &callback_uri, flow_state.pkce_verifier.as_deref()).await?;

            let userinfo_url = &config.token_url.replace("/token", "/userinfo");
            let profile =
                oauth2_provider::fetch_userinfo(userinfo_url, &tokens.access_token).await?;

            let sub = profile
                .get("id")
                .or_else(|| profile.get("sub"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            SubjectInfo {
                subject_type: "user".to_string(),
                properties: serde_json::json!({
                    "provider": provider_name,
                    "sub": sub,
                    "profile": profile,
                }),
            }
        }
        ProviderConfig::Oidc(config) => {
            let tokens =
                oauth2_provider::exchange_code(&config.oauth2, &query.code, &callback_uri, flow_state.pkce_verifier.as_deref()).await?;

            // If we got an id_token, validate it
            if let Some(id_token) = &tokens.id_token {
                let claims =
                    crate::provider::oidc::validate_id_token(id_token, config).await?;
                SubjectInfo {
                    subject_type: "user".to_string(),
                    properties: serde_json::json!({
                        "provider": provider_name,
                        "sub": claims.sub,
                        "email": claims.email,
                        "name": claims.name,
                    }),
                }
            } else {
                // Fall back to userinfo endpoint
                let userinfo_url =
                    config.oauth2.token_url.replace("/token", "/userinfo");
                let profile =
                    oauth2_provider::fetch_userinfo(&userinfo_url, &tokens.access_token).await?;
                let sub = profile
                    .get("sub")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                SubjectInfo {
                    subject_type: "user".to_string(),
                    properties: serde_json::json!({
                        "provider": provider_name,
                        "sub": sub,
                        "profile": profile,
                    }),
                }
            }
        }
        _ => {
            return Err(OAuthError::InvalidRequest(
                "GET callback only supported for OAuth2/OIDC providers".to_string(),
            ));
        }
    };

    // Generate authorization code and redirect back to client
    issue_auth_code_and_redirect(&app, &flow_state.session_key, &subject_info).await
}

/// Combined form data for POST callback
#[derive(Debug, Deserialize)]
pub struct CallbackForm {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub destination: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub session: String,
}

/// Handle POST /:provider/callback (password/code form submission)
async fn provider_callback_post_handler<S: StorageAdapter>(
    State(app): State<AppState<S>>,
    Path(provider_name): Path<String>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<CallbackForm>,
) -> Result<Response, OAuthError> {
    let provider_config = app
        .providers
        .get(&provider_name)
        .ok_or_else(|| OAuthError::InvalidRequest(format!("Unknown provider: {}", provider_name)))?;

    match provider_config {
        ProviderConfig::Password(config) => {
            let ip = crate::ratelimit::middleware::extract_client_ip(
                &headers,
                &app.config.proxy,
            );
            let identifier =
                crate::ratelimit::middleware::get_rate_limit_identifier(None, ip.as_deref());
            if let Err(err) = crate::ratelimit::middleware::check_rate_limit(
                app.storage.as_ref(),
                &app.config.rate_limit,
                Endpoint::PasswordLogin,
                &identifier,
            )
            .await
            {
                return Ok(err.into_response());
            }

            let session_key = extract_session_key(&headers, &form.session)?;

            let email = form.email.clone();
            let password = form.password.clone();
            let action = form.action.clone();

            let result = if action == "register" {
                let reg_result = crate::provider::password::register(
                    app.storage.as_ref(),
                    &email,
                    &password,
                    config,
                )
                .await;

                match reg_result {
                    Ok(_result) => {
                        crate::provider::password::login(
                            app.storage.as_ref(),
                            &email,
                            &password,
                            false,
                        )
                        .await
                    }
                    Err(e) => Err(e),
                }
            } else {
                crate::provider::password::login(
                    app.storage.as_ref(),
                    &email,
                    &password,
                    config.require_verification,
                )
                .await
            };

            match result {
                Ok(subject_info) => {
                    issue_auth_code_and_redirect(&app, &session_key, &subject_info).await
                }
                Err(e) => {
                    let mode = if action == "register" {
                        ui::password::PasswordFormMode::Register
                    } else {
                        ui::password::PasswordFormMode::Login
                    };
                    let html =
                        ui::password::render_password_form(mode, Some(&e.description()), Some(&format!("/{}/callback", provider_name)), Some(&session_key));
                    Ok(Html(html).into_response())
                }
            }
        }
        ProviderConfig::Code(config) => {
            let ip = crate::ratelimit::middleware::extract_client_ip(
                &headers,
                &app.config.proxy,
            );
            let identifier =
                crate::ratelimit::middleware::get_rate_limit_identifier(None, ip.as_deref());
            if let Err(err) = crate::ratelimit::middleware::check_rate_limit(
                app.storage.as_ref(),
                &app.config.rate_limit,
                Endpoint::CodeVerify,
                &identifier,
            )
            .await
            {
                return Ok(err.into_response());
            }

            let session_key = extract_session_key(&headers, &form.session)?;

            if form.action == "request" || form.code.is_empty() {
                // Request a new code
                match crate::provider::code::request_code(
                    app.storage.as_ref(),
                    &form.destination,
                    config,
                )
                .await
                {
                    Ok(_code) => {
                        // In production, send the code via email/SMS here.
                        // Show the verify form.
                        let masked = mask_destination(&form.destination);
                        let html = ui::code::render_code_form(&masked, None);
                        Ok(Html(html).into_response())
                    }
                    Err(e) => {
                        let html =
                            ui::code::render_code_request_form(Some(&e.description()));
                        Ok(Html(html).into_response())
                    }
                }
            } else {
                // Verify the code
                match crate::provider::code::verify_code(
                    app.storage.as_ref(),
                    &form.destination,
                    &form.code,
                )
                .await
                {
                    Ok(subject_info) => {
                        issue_auth_code_and_redirect(&app, &session_key, &subject_info)
                            .await
                    }
                    Err(e) => {
                        let masked = mask_destination(&form.destination);
                        let html = ui::code::render_code_form(&masked, Some(&e.description()));
                        Ok(Html(html).into_response())
                    }
                }
            }
        }
        _ => Err(OAuthError::InvalidRequest(
            "POST callback only supported for password/code providers".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_client_id_from_request(req: &Request) -> Option<String> {
    if let Some(auth) = req.headers().get("Authorization").and_then(|v| v.to_str().ok()) {
        if let Ok((client_id, _)) = crate::client::parse_basic_auth(Some(auth)) {
            return Some(client_id);
        }
    }

    if let Some(query) = req.uri().query() {
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            if key == "client_id" {
                let client_id = value.into_owned();
                if !client_id.is_empty() {
                    return Some(client_id);
                }
            }
        }
    }

    None
}

/// Extract session key from cookie or form field
fn extract_session_key(headers: &HeaderMap, form_session: &str) -> Result<String, OAuthError> {
    if !form_session.is_empty() {
        return Ok(form_session.to_string());
    }

    // Try to extract from cookie
    if let Some(cookie_header) = headers.get("Cookie") {
        if let Ok(cookies) = cookie_header.to_str() {
            for cookie in cookies.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix("irongate_session=") {
                    return Ok(value.to_string());
                }
            }
        }
    }

    Err(OAuthError::InvalidRequest("Missing session".to_string()))
}

/// Generate an authorization code, store it, and redirect back to the client.
async fn issue_auth_code_and_redirect<S: StorageAdapter>(
    app: &AppState<S>,
    session_key: &str,
    subject_info: &SubjectInfo,
) -> Result<Response, OAuthError> {
    // Load the original authorize session
    let session_value = app
        .storage
        .get(&["oauth:session", session_key])
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidGrant("Session expired".to_string()))?;

    let session: crate::oauth::authorize::AuthorizeSession =
        serde_json::from_value(session_value)
            .map_err(|e| OAuthError::ServerError(format!("Corrupt session: {}", e)))?;

    // Delete session (single use)
    let _ = app.storage.remove(&["oauth:session", session_key]).await;

    // Compute subject ID
    let subject = Subject::new(
        &subject_info.subject_type,
        subject_info.properties.clone(),
    );
    let subject_id = subject.id();

    // Generate authorization code
    let code = generate_random_string(32);
    let code_data = serde_json::json!({
        "client_id": session.client_id,
        "redirect_uri": session.redirect_uri,
        "subject": subject_id,
        "subject_type": subject_info.subject_type,
        "properties": subject_info.properties,
        "code_challenge": session.code_challenge,
        "scope": session.scope,
    });

    let code_ttl = app.config.tokens.code_ttl;
    let expiry = chrono::Utc::now() + chrono::Duration::seconds(code_ttl as i64);
    app.storage
        .set(&["oauth:code", &code], code_data, Some(expiry))
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    // Redirect back to client
    let mut redirect_url =
        url::Url::parse(&session.redirect_uri).map_err(|e| {
            OAuthError::ServerError(format!("Invalid redirect URI: {}", e))
        })?;
    redirect_url
        .query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &session.state);

    Ok(Redirect::to(redirect_url.as_str()).into_response())
}

/// Mask an email/phone for display (e.g., "j***@example.com")
fn mask_destination(dest: &str) -> String {
    if let Some(at) = dest.find('@') {
        if at > 1 {
            let first = &dest[..1];
            let domain = &dest[at..];
            format!("{}***{}", first, domain)
        } else {
            format!("***{}", &dest[at..])
        }
    } else if dest.len() > 4 {
        let last4 = &dest[dest.len() - 4..];
        format!("***{}", last4)
    } else {
        "***".to_string()
    }
}
