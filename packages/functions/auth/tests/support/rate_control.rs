//! Counter faults/gates over a real adapter; non-counter reads expose protected-work admission.
#![allow(dead_code)]
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Barrier;

pub enum Fault {
    Read,
    Write,
    LostResponse,
    Conflict,
    Race(usize),
    Observe,
}
pub struct RateControl {
    inner: Arc<dyn StorageAdapter>,
    dimension: &'static str,
    fault: Fault,
    gate: Barrier,
    pub reads: AtomicUsize,
    pub writes: AtomicUsize,
    pub protected_reads: AtomicUsize,
}
impl RateControl {
    pub fn new(inner: Arc<dyn StorageAdapter>, dimension: &'static str, fault: Fault) -> Arc<Self> {
        let racers = match fault {
            Fault::Race(n) => n,
            _ => 1,
        };
        Arc::new(Self {
            inner,
            dimension,
            fault,
            gate: Barrier::new(racers),
            reads: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            protected_reads: AtomicUsize::new(0),
        })
    }
    fn matches(&self, key: &[&str]) -> bool {
        key.first() == Some(&"ratelimit")
            && key.get(2).is_some_and(|id| id.starts_with(self.dimension))
    }
}
#[async_trait]
impl StorageAdapter for RateControl {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        if key.first() != Some(&"ratelimit") {
            self.protected_reads.fetch_add(1, Ordering::SeqCst);
        }
        if self.matches(key) {
            let turn = self.reads.fetch_add(1, Ordering::SeqCst);
            if matches!(self.fault, Fault::Read) {
                return Err(StorageError::DynamoDB("injected read failure".into()));
            }
            let value = self.inner.get(key).await?;
            if let Fault::Race(n) = self.fault {
                if turn < n {
                    tokio::time::timeout(std::time::Duration::from_secs(10), self.gate.wait())
                        .await
                        .expect("all counter readers arrive");
                }
            }
            return Ok(value);
        }
        self.inner.get(key).await
    }
    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        if self.matches(key) {
            self.writes.fetch_add(1, Ordering::SeqCst);
            match self.fault {
                Fault::Write => {
                    return Err(StorageError::DynamoDB("injected write failure".into()))
                }
                Fault::Conflict => return Ok(false),
                _ => {}
            }
        }
        let result = self
            .inner
            .compare_and_set(key, expected, value, expiry)
            .await?;
        if self.matches(key) && matches!(self.fault, Fault::LostResponse) && result {
            return Err(StorageError::DynamoDB("injected lost response".into()));
        }
        Ok(result)
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
    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        self.inner.transact(operations).await
    }
}
