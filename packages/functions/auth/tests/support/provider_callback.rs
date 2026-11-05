//! Inject failures only at provider issuance, after real state/session consumption.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug)]
pub enum Failure {
    Condition,
    Exists,
    Operational,
    Conflict,
}

impl Failure {
    fn error(self) -> StorageError {
        let sensitive = "private-provider-subject private@example.com raw-code sdk-detail";
        match self {
            Self::Condition => StorageError::ConditionFailed(sensitive.into()),
            Self::Exists => StorageError::AlreadyExists(sensitive.into()),
            Self::Operational => StorageError::DynamoDB(sensitive.into()),
            Self::Conflict => StorageError::TransactionConflict,
        }
    }
}

pub struct FaultStorage<S> {
    pub inner: S,
    pub on_prepare: bool,
    pub failure: Failure,
    pub failures: AtomicUsize,
}

pub fn issues_code(operations: &[TransactOperation]) -> bool {
    operations.iter().any(|operation| {
        matches!(operation, TransactOperation::Put { key, .. } if key.first().is_some_and(|pk| pk == "oauth:code"))
    })
}

#[async_trait]
impl<S: StorageAdapter> StorageAdapter for FaultStorage<S> {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        if self.on_prepare && key.first().is_some_and(|pk| pk.starts_with("identity:")) {
            self.failures.fetch_add(1, Ordering::SeqCst);
            return Err(self.failure.error());
        }
        self.inner.get(key).await
    }

    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        if !self.on_prepare && issues_code(&operations) {
            self.failures.fetch_add(1, Ordering::SeqCst);
            return Err(self.failure.error());
        }
        self.inner.transact(operations).await
    }

    async fn set(
        &self,
        key: &[&str],
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError> {
        self.inner.set(key, value, expiry).await
    }

    async fn remove(&self, key: &[&str]) -> Result<(), StorageError> {
        self.inner.remove(key).await
    }

    async fn query_prefix(
        &self,
        prefix: &[&str],
    ) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
        self.inner.query_prefix(prefix).await
    }

    async fn query_prefix_page(
        &self,
        prefix: &[&str],
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError> {
        self.inner.query_prefix_page(prefix, limit, cursor).await
    }

    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        self.inner
            .compare_and_set(key, expected, value, expiry)
            .await
    }
}
