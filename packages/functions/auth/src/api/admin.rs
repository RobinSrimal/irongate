//! IAM-protected account lifecycle admin API.

use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use lambda_http::request::RequestContext;
use serde::Serialize;

use crate::audit::{self, AuditEvent};
use crate::config::AppState;
use crate::core::subjects::Subject;
use crate::error::StorageError;
use crate::storage::StorageAdapter;
use crate::store::records::{AccountRecord, AccountStatus};
use crate::store::AuthStore;

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
    revoked_refresh_families: usize,
}

#[derive(Debug)]
enum AdminApiError {
    Forbidden,
    NotFound,
    Conflict(String),
    Storage(StorageError),
}

pub fn create_admin_router<S: StorageAdapter + Clone + 'static>(state: AppState<S>) -> Router {
    Router::new()
        .route("/_admin/users/:subject", get(get_user::<S>))
        .route("/_admin/users/:subject/disable", post(disable_user::<S>))
        .route(
            "/_admin/users/:subject/revoke-sessions",
            post(revoke_user_sessions::<S>),
        )
        .layer(middleware::from_fn(require_iam_context))
        .with_state(state)
}

async fn require_iam_context(req: Request, next: Next) -> Response {
    if request_has_iam_authorizer(&req) {
        next.run(req).await
    } else {
        AdminApiError::Forbidden.into_response()
    }
}

fn request_has_iam_authorizer(req: &Request) -> bool {
    req.extensions()
        .get::<RequestContext>()
        .and_then(|context| context.authorizer())
        .and_then(|authorizer| authorizer.iam.as_ref())
        .map_or(false, |iam| {
            iam.user_arn
                .as_deref()
                .map_or(false, |value| !value.is_empty())
                || iam
                    .caller_id
                    .as_deref()
                    .map_or(false, |value| !value.is_empty())
                || iam
                    .user_id
                    .as_deref()
                    .map_or(false, |value| !value.is_empty())
        })
}

async fn get_user<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    Path(subject): Path<String>,
) -> Result<Json<AdminAccountResponse>, AdminApiError> {
    let store = AuthStore::new(app.storage.clone());
    let subject = Subject::from_persisted(subject);
    let account = store
        .get_account(&subject)
        .await
        .map_err(AdminApiError::Storage)?
        .ok_or(AdminApiError::NotFound)?;

    let mut event = AuditEvent::new("admin_account_read");
    event.subject = Some(subject.as_str().to_string());
    let _ = audit::record_event(app.storage.as_ref(), event).await;

    Ok(Json(account_response(account)))
}

async fn disable_user<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    Path(subject): Path<String>,
) -> Result<Json<AdminMutationResponse>, AdminApiError> {
    let store = AuthStore::new(app.storage.clone());
    let subject = Subject::from_persisted(subject);
    let account = store
        .disable_account(&subject)
        .await
        .map_err(map_lifecycle_storage_error)?;
    let revoked = store
        .revoke_refresh_tokens_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;

    let mut event = AuditEvent::new("admin_account_disabled");
    event.subject = Some(subject.as_str().to_string());
    event.detail = Some(format!("revoked_refresh_families={revoked}"));
    let _ = audit::record_event(app.storage.as_ref(), event).await;

    Ok(Json(AdminMutationResponse {
        subject: account.subject,
        status: account_status(&account.status),
        disabled_at: account.disabled_at,
        revoked_refresh_families: revoked,
    }))
}

async fn revoke_user_sessions<S: StorageAdapter + Clone>(
    State(app): State<AppState<S>>,
    Path(subject): Path<String>,
) -> Result<Json<AdminMutationResponse>, AdminApiError> {
    let store = AuthStore::new(app.storage.clone());
    let subject = Subject::from_persisted(subject);
    let account = store
        .get_account(&subject)
        .await
        .map_err(AdminApiError::Storage)?
        .ok_or(AdminApiError::NotFound)?;
    let revoked = store
        .revoke_refresh_tokens_for_subject(subject.as_str())
        .await
        .map_err(AdminApiError::Storage)?;

    let mut event = AuditEvent::new("admin_subject_sessions_revoked");
    event.subject = Some(subject.as_str().to_string());
    event.detail = Some(format!("revoked_refresh_families={revoked}"));
    let _ = audit::record_event(app.storage.as_ref(), event).await;

    Ok(Json(AdminMutationResponse {
        subject: account.subject,
        status: account_status(&account.status),
        disabled_at: account.disabled_at,
        revoked_refresh_families: revoked,
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
