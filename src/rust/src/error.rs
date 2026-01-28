//! Error types for Irongate OAuth 2.0 server.
//!
//! Provides comprehensive error handling for OAuth, authentication,
//! and internal server errors.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Result type alias for Irongate operations
pub type Result<T> = std::result::Result<T, IrongateError>;

/// Top-level error type for all Irongate operations
#[derive(Debug, Error)]
pub enum IrongateError {
    #[error(transparent)]
    OAuth(#[from] OAuthError),

    #[error(transparent)]
    Auth(#[from] AuthError),

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// OAuth 2.0 specific errors (RFC 6749)
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("invalid_request: {0}")]
    InvalidRequest(String),

    #[error("invalid_client: {0}")]
    InvalidClient(String),

    #[error("invalid_grant: {0}")]
    InvalidGrant(String),

    #[error("unauthorized_client: {0}")]
    UnauthorizedClient(String),

    #[error("access_denied: {0}")]
    AccessDenied(String),

    #[error("unsupported_grant_type: {0}")]
    UnsupportedGrantType(String),

    #[error("unsupported_response_type: {0}")]
    UnsupportedResponseType(String),

    #[error("invalid_redirect_uri: {0}")]
    InvalidRedirectUri(String),

    #[error("invalid_scope: {0}")]
    InvalidScope(String),

    #[error("server_error: {0}")]
    ServerError(String),

    #[error("temporarily_unavailable: {0}")]
    TemporarilyUnavailable(String),
}

/// Authentication errors for admin/management API
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("missing API key")]
    MissingApiKey,

    #[error("invalid API key format")]
    InvalidKeyFormat,

    #[error("invalid API key")]
    InvalidApiKey,

    #[error("insufficient permissions")]
    InsufficientPermissions,

    #[error("rate limit exceeded")]
    RateLimitExceeded {
        limit: u32,
        window_seconds: u64,
        retry_after: u64,
    },
}

/// Storage layer errors
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("item not found: {0}")]
    NotFound(String),

    #[error("item already exists: {0}")]
    AlreadyExists(String),

    #[error("condition check failed: {0}")]
    ConditionFailed(String),

    #[error("DynamoDB error: {0}")]
    DynamoDB(String),
}

/// OAuth error response body
#[derive(Debug, Serialize)]
pub struct OAuthErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
}

impl OAuthError {
    /// Get the error code for OAuth responses
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::InvalidClient(_) => "invalid_client",
            Self::InvalidGrant(_) => "invalid_grant",
            Self::UnauthorizedClient(_) => "unauthorized_client",
            Self::AccessDenied(_) => "access_denied",
            Self::UnsupportedGrantType(_) => "unsupported_grant_type",
            Self::UnsupportedResponseType(_) => "unsupported_response_type",
            Self::InvalidRedirectUri(_) => "invalid_redirect_uri",
            Self::InvalidScope(_) => "invalid_scope",
            Self::ServerError(_) => "server_error",
            Self::TemporarilyUnavailable(_) => "temporarily_unavailable",
        }
    }

    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::InvalidClient(_) => StatusCode::UNAUTHORIZED,
            Self::InvalidGrant(_) => StatusCode::BAD_REQUEST,
            Self::UnauthorizedClient(_) => StatusCode::FORBIDDEN,
            Self::AccessDenied(_) => StatusCode::FORBIDDEN,
            Self::UnsupportedGrantType(_) => StatusCode::BAD_REQUEST,
            Self::UnsupportedResponseType(_) => StatusCode::BAD_REQUEST,
            Self::InvalidRedirectUri(_) => StatusCode::BAD_REQUEST,
            Self::InvalidScope(_) => StatusCode::BAD_REQUEST,
            Self::ServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TemporarilyUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Get the error description
    pub fn description(&self) -> String {
        match self {
            Self::InvalidRequest(msg)
            | Self::InvalidClient(msg)
            | Self::InvalidGrant(msg)
            | Self::UnauthorizedClient(msg)
            | Self::AccessDenied(msg)
            | Self::UnsupportedGrantType(msg)
            | Self::UnsupportedResponseType(msg)
            | Self::InvalidRedirectUri(msg)
            | Self::InvalidScope(msg)
            | Self::ServerError(msg)
            | Self::TemporarilyUnavailable(msg) => msg.clone(),
        }
    }
}

impl IntoResponse for OAuthError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = OAuthErrorResponse {
            error: self.error_code().to_string(),
            error_description: Some(self.description()),
        };
        (status, Json(body)).into_response()
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::MissingApiKey => (StatusCode::UNAUTHORIZED, "Missing API key"),
            Self::InvalidKeyFormat => (StatusCode::BAD_REQUEST, "Invalid API key format"),
            Self::InvalidApiKey => (StatusCode::UNAUTHORIZED, "Invalid API key"),
            Self::InsufficientPermissions => (StatusCode::FORBIDDEN, "Insufficient permissions"),
            Self::RateLimitExceeded { retry_after, .. } => {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [("Retry-After", retry_after.to_string())],
                    "Rate limit exceeded",
                )
                    .into_response();
            }
        };
        (status, message).into_response()
    }
}

impl IntoResponse for IrongateError {
    fn into_response(self) -> Response {
        match self {
            Self::OAuth(e) => e.into_response(),
            Self::Auth(e) => e.into_response(),
            Self::Storage(e) => {
                tracing::error!("Storage error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
            Self::Internal(e) => {
                tracing::error!("Internal error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    }
}
