//! Safe classification of DynamoDB transaction failures.

use aws_sdk_dynamodb::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;

use crate::error::StorageError;

pub(super) fn transaction_error(error: SdkError<TransactWriteItemsError>) -> StorageError {
    if let Some(TransactWriteItemsError::TransactionCanceledException(cancelled)) =
        error.as_service_error()
    {
        let reasons = cancelled.cancellation_reasons();
        // Any unknown/operational reason prevents treating this as a credential rejection.
        let known = !reasons.is_empty()
            && reasons.iter().all(|reason| {
                matches!(
                    reason.code(),
                    Some("None" | "ConditionalCheckFailed" | "TransactionConflict")
                )
            });
        if known {
            if reasons
                .iter()
                .any(|reason| reason.code() == Some("TransactionConflict"))
            {
                return StorageError::TransactionConflict;
            }
            if reasons
                .iter()
                .any(|reason| reason.code() == Some("ConditionalCheckFailed"))
            {
                return StorageError::ConditionFailed("transaction condition failed".into());
            }
        }
    }

    // Never format the SDK error: messages, unknown codes and cancellation items can
    // contain auth data. These categories are a closed, application-owned vocabulary.
    let category = match &error {
        SdkError::ConstructionFailure(_) => "construction",
        SdkError::TimeoutError(_) => "timeout",
        SdkError::DispatchFailure(_) => "dispatch",
        SdkError::ResponseError(_) => "response",
        SdkError::ServiceError(service) => match service.err().code() {
            Some("TransactionCanceledException") => "unclassified_cancellation",
            Some("AccessDeniedException") => "access_denied",
            Some(
                "ProvisionedThroughputExceededException"
                | "ThrottlingException"
                | "RequestLimitExceeded",
            ) => "throttled",
            Some("ValidationException") => "validation",
            _ => "service",
        },
        _ => "unknown",
    };
    tracing::warn!(category, "auth storage transaction failed");
    StorageError::DynamoDB("transaction failed".into())
}
