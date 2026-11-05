//! Safe storage failures shared by provider callbacks.

use crate::error::{OAuthError, StorageError};

pub(super) fn map_storage_error(error: StorageError) -> OAuthError {
    match error {
        StorageError::ConditionFailed(_) | StorageError::AlreadyExists(_) => {
            OAuthError::InvalidGrant("provider sign-in is no longer valid".into())
        }
        _ => OAuthError::ServerError("provider sign-in unavailable".into()),
    }
}
