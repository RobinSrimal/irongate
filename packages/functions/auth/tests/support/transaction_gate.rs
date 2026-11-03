//! Pause before a real transaction so tests can commit a competing writer first.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

pub struct PausedStorage<S> {
    inner: S,
    pending: mpsc::Sender<PendingTransaction>,
}

pub struct TransactionGate(mpsc::Receiver<PendingTransaction>);

pub struct PendingTransaction {
    pub operations: Vec<TransactOperation>,
    resume: oneshot::Sender<()>,
}

impl<S> PausedStorage<S> {
    pub fn new(inner: S) -> (Self, TransactionGate) {
        let (pending, receiver) = mpsc::channel(1);
        (Self { inner, pending }, TransactionGate(receiver))
    }
}

impl TransactionGate {
    pub async fn wait(&mut self) -> PendingTransaction {
        tokio::time::timeout(Duration::from_secs(5), self.0.recv())
            .await
            .expect("transaction reached gate")
            .expect("transaction producer alive")
    }
}

impl PendingTransaction {
    pub fn commit(self) {
        self.resume.send(()).expect("transaction is waiting");
    }
}

#[async_trait]
impl<S: StorageAdapter> StorageAdapter for PausedStorage<S> {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        self.inner.get(key).await
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
    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        let (resume, wait) = oneshot::channel();
        self.pending
            .send(PendingTransaction {
                operations: operations.clone(),
                resume,
            })
            .await
            .map_err(|_| StorageError::DynamoDB("test gate closed".into()))?;
        tokio::time::timeout(Duration::from_secs(10), wait)
            .await
            .map_err(|_| StorageError::DynamoDB("test gate timeout".into()))?
            .map_err(|_| StorageError::DynamoDB("test gate cancelled".into()))?;
        self.inner.transact(operations).await
    }
}
