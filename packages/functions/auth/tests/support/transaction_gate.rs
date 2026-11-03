//! Pause before a real transaction so tests can commit a competing writer first.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

type TransactionSelector = dyn Fn(&[TransactOperation]) -> bool + Send + Sync;

pub struct PausedStorage<S> {
    inner: S,
    pending: mpsc::Sender<PendingTransaction>,
    select: Box<TransactionSelector>,
    read_key: Option<Vec<String>>,
    fired: AtomicBool,
}

pub struct TransactionGate(mpsc::Receiver<PendingTransaction>);

pub struct PendingTransaction {
    pub operations: Vec<TransactOperation>,
    resume: oneshot::Sender<Action>,
}

enum Action {
    Commit,
    FailBeforeDispatch,
    LoseResponse,
}

impl<S> PausedStorage<S> {
    pub fn new(inner: S) -> (Self, TransactionGate) {
        Self::when(inner, |_| true)
    }

    pub fn when(
        inner: S,
        select: impl Fn(&[TransactOperation]) -> bool + Send + Sync + 'static,
    ) -> (Self, TransactionGate) {
        let (pending, receiver) = mpsc::channel(1);
        (
            Self {
                inner,
                pending,
                select: Box::new(select),
                read_key: None,
                fired: AtomicBool::new(false),
            },
            TransactionGate(receiver),
        )
    }

    pub fn before_read(inner: S, key: Vec<String>) -> (Self, TransactionGate) {
        let (mut storage, gate) = Self::when(inner, |_| false);
        storage.read_key = Some(key);
        (storage, gate)
    }

    async fn pause(&self, operations: Vec<TransactOperation>) -> Result<Action, StorageError> {
        let (resume, wait) = oneshot::channel();
        self.pending
            .send(PendingTransaction { operations, resume })
            .await
            .map_err(|_| StorageError::DynamoDB("test gate closed".into()))?;
        tokio::time::timeout(Duration::from_secs(10), wait)
            .await
            .map_err(|_| StorageError::DynamoDB("test gate timeout".into()))?
            .map_err(|_| StorageError::DynamoDB("test gate cancelled".into()))
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
        assert!(
            self.resume.send(Action::Commit).is_ok(),
            "operation is waiting"
        );
    }
    pub fn fail_before_dispatch(self) {
        assert!(
            self.resume.send(Action::FailBeforeDispatch).is_ok(),
            "operation is waiting"
        );
    }
    pub fn lose_response(self) {
        assert!(
            self.resume.send(Action::LoseResponse).is_ok(),
            "operation is waiting"
        );
    }
}

#[async_trait]
impl<S: StorageAdapter> StorageAdapter for PausedStorage<S> {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        if self
            .read_key
            .as_ref()
            .is_some_and(|expected| expected.iter().map(String::as_str).eq(key.iter().copied()))
            && !self.fired.swap(true, Ordering::SeqCst)
            && !matches!(self.pause(vec![]).await?, Action::Commit)
        {
            return Err(StorageError::DynamoDB("test read interrupted".into()));
        }
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
        if (self.select)(&operations) && !self.fired.swap(true, Ordering::SeqCst) {
            match self.pause(operations.clone()).await? {
                Action::Commit => {}
                Action::FailBeforeDispatch => {
                    return Err(StorageError::DynamoDB(
                        "test unavailable before dispatch".into(),
                    ))
                }
                Action::LoseResponse => {
                    self.inner.transact(operations).await?;
                    return Err(StorageError::DynamoDB(
                        "test response lost after commit".into(),
                    ));
                }
            }
        }
        self.inner.transact(operations).await
    }
}
