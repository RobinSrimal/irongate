//! Storage adapter trait definition.
//!
//! Defines the interface for all storage backends.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;

use crate::error::StorageError;

/// Storage adapter trait for key-value storage.
///
/// Keys are arrays of strings that get encoded into partition key (pk)
/// and sort key (sk) for DynamoDB.
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    /// Get a value by key.
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError>;

    /// Set a value with optional expiry.
    ///
    /// If `expiry` is `Some`, the value will be automatically deleted
    /// after the specified time (via DynamoDB TTL).
    async fn set(
        &self,
        key: &[&str],
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError>;

    /// Remove a value by key.
    async fn remove(&self, key: &[&str]) -> Result<(), StorageError>;

    /// Query all values with a bounded key prefix.
    ///
    /// Returns a list of (key, value) pairs.
    async fn query_prefix(
        &self,
        prefix: &[&str],
    ) -> Result<Vec<(Vec<String>, Value)>, StorageError>;

    /// Query a bounded key prefix with pagination support.
    ///
    /// Returns up to `limit` items and an opaque cursor for the next page.
    /// Pass `cursor: None` to start from the beginning.
    async fn query_prefix_page(
        &self,
        prefix: &[&str],
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError>;

    /// Atomically check and set a value.
    ///
    /// Only sets the value if the current value matches `expected`.
    /// Returns `true` if the value was set, `false` if the condition failed.
    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        new_value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError>;

    /// Execute a transaction with multiple operations.
    ///
    /// All operations succeed or fail atomically.
    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError>;
}

/// Transaction operation for atomic multi-item updates
#[derive(Debug, Clone)]
pub enum TransactOperation {
    /// Put a new item or replace existing
    Put {
        key: Vec<String>,
        value: Value,
        expiry: Option<DateTime<Utc>>,
    },
    /// Delete an item
    Delete { key: Vec<String> },
    /// Check a condition without modifying
    ConditionCheck {
        key: Vec<String>,
        condition: TransactCondition,
    },
    /// Update an existing item
    Update {
        key: Vec<String>,
        updates: Value,
        condition: Option<TransactCondition>,
    },
}

/// Condition for transactional operations
#[derive(Debug, Clone)]
pub enum TransactCondition {
    /// Item must exist
    Exists,
    /// Item must not exist
    NotExists,
    /// Attribute must equal value
    AttributeEquals { name: String, value: Value },
}

#[async_trait]
impl<T> StorageAdapter for Arc<T>
where
    T: StorageAdapter + ?Sized,
{
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        self.as_ref().get(key).await
    }

    async fn set(
        &self,
        key: &[&str],
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError> {
        self.as_ref().set(key, value, expiry).await
    }

    async fn remove(&self, key: &[&str]) -> Result<(), StorageError> {
        self.as_ref().remove(key).await
    }

    async fn query_prefix(
        &self,
        prefix: &[&str],
    ) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
        self.as_ref().query_prefix(prefix).await
    }

    async fn query_prefix_page(
        &self,
        prefix: &[&str],
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError> {
        self.as_ref().query_prefix_page(prefix, limit, cursor).await
    }

    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        new_value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        self.as_ref()
            .compare_and_set(key, expected, new_value, expiry)
            .await
    }

    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        self.as_ref().transact(operations).await
    }
}
