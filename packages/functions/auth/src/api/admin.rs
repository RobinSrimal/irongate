//! IAM-protected account lifecycle admin API.

use axum::{
    extract::{Extension, Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use lambda_http::request::RequestContext;
use serde::Serialize;

use crate::audit::{AuditEmitter, AuditEvent};
use crate::config::account_lifecycle::AccountLifecycleConfig;
use crate::core::subjects::Subject;
use crate::error::StorageError;
use crate::store::records::{AccountRecord, AccountStatus};
use crate::store::AuthStore;

#[derive(Clone)]
pub struct AdminAppState {
    pub store: AuthStore,
    pub lifecycle: AccountLifecycleConfig,
    pub audit: AuditEmitter,
}

#[derive(Debug, Serialize)]
struct AdminAccountResponse {
    subject: String,
    status: &'static str,
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
struct AdminMutationResponse {
    subject: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    revoked_refresh_families: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_identities: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_password_users: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_password_secrets: Option<usize>,
}

#[derive(Debug)]
enum AdminApiError {
    Forbidden,
    NotFound,
    Conflict(String),
    Storage(StorageError),
}

pub fn create_admin_router(state: AdminAppState) -> Router {
    Router::new()
        .route("/_admin/users/:subject", get(get_user))
        .route("/_admin/users/:subject/disable", post(disable_user))
        .route("/_admin/users/:subject/enable", post(enable_user))
        .route("/_admin/users/:subject/delete", post(delete_user))
        .route(
            "/_admin/users/:subject/revoke-sessions",
            post(revoke_user_sessions),
        )
        .layer(middleware::from_fn(require_iam_context))
        .with_state(state)
}

#[derive(Clone)]
struct IamActor(String);

async fn require_iam_context(mut req: Request, next: Next) -> Response {
    if let Some(actor) = iam_actor(&req) {
        req.extensions_mut().insert(IamActor(actor));
        next.run(req).await
    } else {
        AdminApiError::Forbidden.into_response()
    }
}

fn iam_actor(req: &Request) -> Option<String> {
    req.extensions()
        .get::<RequestContext>()
        .and_then(|context| context.authorizer())
        .and_then(|authorizer| authorizer.iam.as_ref())
        .and_then(|iam| {
            [
                iam.user_arn.as_deref(),
                iam.caller_id.as_deref(),
                iam.user_id.as_deref(),
            ]
            .into_iter()
            .flatten()
            .find(|value| !value.is_empty())
            .map(str::to_owned)
        })
}

async fn get_user(
    State(app): State<AdminAppState>,
    Path(subject): Path<String>,
    Extension(IamActor(actor)): Extension<IamActor>,
) -> Result<Json<AdminAccountResponse>, AdminApiError> {
    let subject = Subject::from_persisted(subject);
    let account = app
        .store
        .get_account(&subject)
        .await
        .map_err(AdminApiError::Storage)?
        .ok_or(AdminApiError::NotFound)?;

    app.audit.emit(AuditEvent::AdminAccountRead {
        actor: &actor,
        subject: &account.subject,
    });

    Ok(Json(account_response(account)))
}

async fn disable_user(
    State(app): State<AdminAppState>,
    Path(subject): Path<String>,
    Extension(IamActor(actor)): Extension<IamActor>,
) -> Result<Json<AdminMutationResponse>, AdminApiError> {
    let subject = Subject::from_persisted(subject);
    let account = app
        .store
        .disable_account(&subject)
        .await
        .map_err(map_lifecycle_storage_error)?;
    let revoked = app
        .store
        .revoke_refresh_tokens_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;
    let deleted_password_secrets = app
        .store
        .delete_password_secrets_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;

    app.audit.emit(AuditEvent::AdminAccountDisabled {
        actor: &actor,
        subject: &account.subject,
        revoked_refresh_families: revoked,
        deleted_password_secrets,
    });

    Ok(Json(AdminMutationResponse {
        subject: account.subject,
        status: account_status(&account.status),
        disabled_at: account.disabled_at,
        deleted_at: account.deleted_at,
        revoked_refresh_families: revoked,
        deleted_identities: None,
        deleted_password_users: None,
        deleted_password_secrets: Some(deleted_password_secrets),
    }))
}

async fn enable_user(
    State(app): State<AdminAppState>,
    Path(subject): Path<String>,
    Extension(IamActor(actor)): Extension<IamActor>,
) -> Result<Json<AdminMutationResponse>, AdminApiError> {
    let subject = Subject::from_persisted(subject);
    let account = app
        .store
        .enable_account(&subject)
        .await
        .map_err(map_lifecycle_storage_error)?;
    let revoked = app
        .store
        .revoke_refresh_tokens_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;
    let deleted_password_secrets = app
        .store
        .delete_password_secrets_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;

    app.audit.emit(AuditEvent::AdminAccountEnabled {
        actor: &actor,
        subject: &account.subject,
        revoked_refresh_families: revoked,
        deleted_password_secrets,
    });

    Ok(Json(AdminMutationResponse {
        subject: account.subject,
        status: account_status(&account.status),
        disabled_at: account.disabled_at,
        deleted_at: account.deleted_at,
        revoked_refresh_families: revoked,
        deleted_identities: None,
        deleted_password_users: None,
        deleted_password_secrets: Some(deleted_password_secrets),
    }))
}

async fn delete_user(
    State(app): State<AdminAppState>,
    Path(subject): Path<String>,
    Extension(IamActor(actor)): Extension<IamActor>,
) -> Result<Json<AdminMutationResponse>, AdminApiError> {
    let subject = Subject::from_persisted(subject);
    let outcome = app
        .store
        .delete_account(
            &subject,
            app.lifecycle.deleted_identity_reuse,
            app.lifecycle.deleted_identity_retention_days,
        )
        .await
        .map_err(map_lifecycle_storage_error)?;

    app.audit.emit(AuditEvent::AdminAccountDeleted {
        actor: &actor,
        subject: &outcome.account.subject,
        revoked_refresh_families: outcome.revoked_refresh_families,
        deleted_identities: outcome.deleted_identities,
        deleted_password_users: outcome.deleted_password_users,
        deleted_password_secrets: outcome.deleted_password_secrets,
    });

    Ok(Json(AdminMutationResponse {
        subject: outcome.account.subject,
        status: account_status(&outcome.account.status),
        disabled_at: outcome.account.disabled_at,
        deleted_at: outcome.account.deleted_at,
        revoked_refresh_families: outcome.revoked_refresh_families,
        deleted_identities: Some(outcome.deleted_identities),
        deleted_password_users: Some(outcome.deleted_password_users),
        deleted_password_secrets: Some(outcome.deleted_password_secrets),
    }))
}

async fn revoke_user_sessions(
    State(app): State<AdminAppState>,
    Path(subject): Path<String>,
    Extension(IamActor(actor)): Extension<IamActor>,
) -> Result<Json<AdminMutationResponse>, AdminApiError> {
    let subject = Subject::from_persisted(subject);
    let account = app
        .store
        .get_account(&subject)
        .await
        .map_err(AdminApiError::Storage)?
        .ok_or(AdminApiError::NotFound)?;
    let revoked = app
        .store
        .revoke_refresh_tokens_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;

    app.audit.emit(AuditEvent::AdminSubjectSessionsRevoked {
        actor: &actor,
        subject: &account.subject,
        revoked_refresh_families: revoked,
    });

    Ok(Json(AdminMutationResponse {
        subject: account.subject,
        status: account_status(&account.status),
        disabled_at: account.disabled_at,
        deleted_at: account.deleted_at,
        revoked_refresh_families: revoked,
        deleted_identities: None,
        deleted_password_users: None,
        deleted_password_secrets: None,
    }))
}

fn account_response(account: AccountRecord) -> AdminAccountResponse {
    AdminAccountResponse {
        subject: account.subject,
        status: account_status(&account.status),
        created_at: account.created_at,
        disabled_at: account.disabled_at,
        deleted_at: account.deleted_at,
    }
}

fn account_status(status: &AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Disabled => "disabled",
        AccountStatus::Deleted => "deleted",
    }
}

fn map_lifecycle_storage_error(err: StorageError) -> AdminApiError {
    match err {
        StorageError::NotFound(_) => AdminApiError::NotFound,
        StorageError::ConditionFailed(message) => AdminApiError::Conflict(message),
        other => AdminApiError::Storage(other),
    }
}

impl IntoResponse for AdminApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden",
                    "error_description": "IAM authorizer context required"
                })),
            )
                .into_response(),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "not_found",
                    "error_description": "account not found"
                })),
            )
                .into_response(),
            Self::Conflict(message) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "conflict",
                    "error_description": message
                })),
            )
                .into_response(),
            Self::Storage(err) => {
                tracing::error!("Admin lifecycle storage error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "server_error",
                        "error_description": "internal server error"
                    })),
                )
                    .into_response()
            }
        }
    }
}
