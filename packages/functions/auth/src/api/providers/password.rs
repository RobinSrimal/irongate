//! Password provider API handlers.

use axum::{extract::State, http::HeaderMap, response::Redirect, Form, Json};
use serde::{Deserialize, Serialize};

use crate::config::{AppState, Endpoint};
use crate::error::{IrongateError, OAuthError};
use crate::providers::password::{
    login_password_user, register_password_user, verify_password_email, PasswordLoginError,
    PasswordLoginInput, PasswordRegistrationError, PasswordRegistrationInput,
    PasswordRegistrationStatus, PasswordVerificationError, PasswordVerificationInput,
    PasswordVerificationStatus,
};
use crate::ratelimit::middleware::{check_rate_limit, extract_client_ip};
use crate::storage::StorageAdapter;
use crate::store::rate_limits::{
    password_email_rate_limit_identifier, source_rate_limit_identifier,
};
use crate::store::AuthStore;

#[derive(Debug, Deserialize)]
pub(crate) struct PasswordRegisterRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct PasswordRegisterResponse {
    status: PasswordRegistrationStatus,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PasswordVerifyRequest {
    token: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct PasswordVerifyResponse {
    status: PasswordVerificationStatus,
    subject: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PasswordLoginRequest {
    session: String,
    email: String,
    password: String,
}

pub(crate) async fn password_register_handler<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    headers: HeaderMap,
    Json(payload): Json<PasswordRegisterRequest>,
) -> Result<Json<PasswordRegisterResponse>, IrongateError> {
    enforce_password_rate_limit(
        &app,
        &headers,
        Endpoint::PasswordRegister,
        Some(&payload.email),
    )
    .await?;

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
    .map_err(map_password_registration_error)
    .map_err(IrongateError::OAuth)?;

    Ok(Json(PasswordRegisterResponse {
        status: outcome.status,
    }))
}

pub(crate) async fn password_verify_handler<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    headers: HeaderMap,
    Json(payload): Json<PasswordVerifyRequest>,
) -> Result<Json<PasswordVerifyResponse>, IrongateError> {
    enforce_password_rate_limit(&app, &headers, Endpoint::PasswordVerify, None).await?;

    let store = AuthStore::new(app.storage.clone());
    let outcome = verify_password_email(
        &store,
        &app.runtime,
        PasswordVerificationInput {
            token: &payload.token,
        },
    )
    .await
    .map_err(map_password_verification_error)
    .map_err(IrongateError::OAuth)?;

    Ok(Json(PasswordVerifyResponse {
        status: outcome.status,
        subject: outcome.subject,
    }))
}

pub(crate) async fn password_login_handler<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    headers: HeaderMap,
    Form(payload): Form<PasswordLoginRequest>,
) -> Result<Redirect, IrongateError> {
    enforce_password_rate_limit(
        &app,
        &headers,
        Endpoint::PasswordLogin,
        Some(&payload.email),
    )
    .await?;

    let store = AuthStore::new(app.storage.clone());
    let outcome = login_password_user(
        &store,
        &app.runtime,
        PasswordLoginInput {
            session: &payload.session,
            email: &payload.email,
            password: &payload.password,
        },
    )
    .await
    .map_err(map_password_login_error)
    .map_err(IrongateError::OAuth)?;

    Ok(Redirect::to(&outcome.redirect_uri))
}

async fn enforce_password_rate_limit<S: StorageAdapter>(
    app: &AppState<S>,
    headers: &HeaderMap,
    endpoint: Endpoint,
    email: Option<&str>,
) -> Result<(), IrongateError> {
    let source = extract_client_ip(headers, &app.config.proxy);
    let identifier = match email {
        Some(email) => password_email_rate_limit_identifier(
            app.runtime.lookup_secret.as_bytes(),
            email,
            source.as_deref(),
        ),
        None => source_rate_limit_identifier(source.as_deref()),
    };

    check_rate_limit(
        app.storage.as_ref(),
        &app.config.rate_limit,
        endpoint,
        &identifier,
    )
    .await
    .map_err(IrongateError::Auth)
}

fn map_password_registration_error(err: PasswordRegistrationError) -> OAuthError {
    match err {
        PasswordRegistrationError::Password(_) => {
            OAuthError::InvalidRequest("invalid registration request".to_string())
        }
        PasswordRegistrationError::EmailAlreadyRegistered => {
            OAuthError::InvalidRequest("email is already registered".to_string())
        }
        PasswordRegistrationError::Storage(storage) => OAuthError::ServerError(storage.to_string()),
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
        PasswordVerificationError::Storage(storage) => OAuthError::ServerError(storage.to_string()),
    }
}

fn map_password_login_error(err: PasswordLoginError) -> OAuthError {
    match err {
        PasswordLoginError::InvalidCredentials
        | PasswordLoginError::EmailNotVerified
        | PasswordLoginError::MissingSubject
        | PasswordLoginError::AccountNotActive => {
            OAuthError::AccessDenied("invalid email or password".to_string())
        }
        PasswordLoginError::InvalidAuthorizeSession => {
            OAuthError::InvalidGrant("authorize session is invalid or expired".to_string())
        }
        PasswordLoginError::InvalidRedirectUri => {
            OAuthError::ServerError("registered redirect URI is invalid".to_string())
        }
        PasswordLoginError::Storage(storage) => OAuthError::ServerError(storage.to_string()),
    }
}
