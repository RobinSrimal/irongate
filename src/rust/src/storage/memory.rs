//! In-memory storage adapter for local development and testing.
//!
//! Uses `std::sync::RwLock<HashMap>` — no extra dependencies needed.
//! NOT suitable for production use (no persistence, single-process only).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::RwLock;

use crate::error::StorageError;

use super::adapter::{StorageAdapter, TransactCondition, TransactOperation};

const SEP: &str = "\x1f";

/// In-memory storage backend for development and testing.
#[derive(Debug)]
pub struct MemoryStorage {
    data: RwLock<BTreeMap<String, Entry>>,
}

#[derive(Debug, Clone)]
struct Entry {
    value: Value,
    expiry: Option<DateTime<Utc>>,
}

impl Entry {
    fn is_expired(&self) -> bool {
        self.expiry.map_or(false, |exp| Utc::now() >= exp)
    }
}

impl Clone for MemoryStorage {
    fn clone(&self) -> Self {
        let data = self.data.read().unwrap().clone();
        Self {
            data: RwLock::new(data),
        }
    }
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(BTreeMap::new()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

fn encode_key(parts: &[&str]) -> String {
    parts.join(SEP)
}

fn decode_key(encoded: &str) -> Vec<String> {
    encoded.split(SEP).map(|s| s.to_string()).collect()
}

#[async_trait]
impl StorageAdapter for MemoryStorage {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        let encoded = encode_key(key);
        let data = self.data.read().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;
        match data.get(&encoded) {
            Some(entry) if !entry.is_expired() => Ok(Some(entry.value.clone())),
            _ => Ok(None),
        }
    }

    async fn set(
        &self,
        key: &[&str],
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError> {
        let encoded = encode_key(key);
        let mut data = self.data.write().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;
        data.insert(encoded, Entry { value, expiry });
        Ok(())
    }

    async fn remove(&self, key: &[&str]) -> Result<(), StorageError> {
        let encoded = encode_key(key);
        let mut data = self.data.write().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;
        data.remove(&encoded);
        Ok(())
    }

    async fn scan(&self, prefix: &[&str]) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
        let prefix_str = encode_key(prefix);
        let data = self.data.read().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;
        let results = data
            .range(prefix_str.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix_str))
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(k, entry)| (decode_key(k), entry.value.clone()))
            .collect();
        Ok(results)
    }

    async fn scan_page(
        &self,
        prefix: &[&str],
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError> {
        let prefix_str = encode_key(prefix);
        let data = self.data.read().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;

        let start = cursor.unwrap_or(&prefix_str);
        let items: Vec<_> = data
            .range(start.to_string()..)
            .take_while(|(k, _)| k.starts_with(&prefix_str))
            .filter(|(k, _)| cursor.map_or(true, |c| k.as_str() > c))
            .filter(|(_, entry)| !entry.is_expired())
            .take((limit + 1) as usize)
            .map(|(k, entry)| (k.clone(), decode_key(k), entry.value.clone()))
            .collect();

        let has_more = items.len() > limit as usize;
        let page: Vec<_> = items
            .iter()
            .take(limit as usize)
            .map(|(_, key, val)| (key.clone(), val.clone()))
            .collect();
        let next_cursor = if has_more {
            items.get(limit as usize - 1).map(|(k, _, _)| k.clone())
        } else {
            None
        };

        Ok((page, next_cursor))
    }

    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        new_value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        let encoded = encode_key(key);
        let mut data = self.data.write().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;

        let current = data.get(&encoded).and_then(|e| {
            if e.is_expired() { None } else { Some(&e.value) }
        });

        match (expected, current) {
            (None, None) | (Some(_), Some(_)) => {
                if let (Some(exp), Some(cur)) = (expected, current) {
                    if exp != cur {
                        return Ok(false);
                    }
                }
                data.insert(encoded, Entry { value: new_value, expiry });
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        let mut data = self.data.write().map_err(|e| StorageError::DynamoDB(format!("lock poisoned: {}", e)))?;

        // Validate all conditions first
        for op in &operations {
            match op {
                TransactOperation::ConditionCheck { key, condition } => {
                    let encoded = key.join(SEP);
                    let exists = data.get(&encoded).map_or(false, |e| !e.is_expired());
                    if !check_condition(condition, exists, data.get(&encoded)) {
                        return Err(StorageError::ConditionFailed("transaction condition failed".into()));
                    }
                }
                TransactOperation::Update { key, condition: Some(cond), .. } => {
                    let encoded = key.join(SEP);
                    let exists = data.get(&encoded).map_or(false, |e| !e.is_expired());
                    if !check_condition(cond, exists, data.get(&encoded)) {
                        return Err(StorageError::ConditionFailed("transaction condition failed".into()));
                    }
                }
                _ => {}
            }
        }

        // Apply all operations
        for op in operations {
            match op {
                TransactOperation::Put { key, value, expiry } => {
                    let encoded = key.join(SEP);
                    data.insert(encoded, Entry { value, expiry });
                }
                TransactOperation::Delete { key } => {
                    let encoded = key.join(SEP);
                    data.remove(&encoded);
                }
                TransactOperation::Update { key, updates, .. } => {
                    let encoded = key.join(SEP);
                    if let Some(entry) = data.get_mut(&encoded) {
                        if let (Value::Object(existing), Value::Object(new_fields)) =
                            (&mut entry.value, updates)
                        {
                            for (k, v) in new_fields {
                                existing.insert(k, v);
                            }
                        }
                    }
                }
                TransactOperation::ConditionCheck { .. } => { /* already checked */ }
            }
        }

        Ok(())
    }
}

fn check_condition(condition: &TransactCondition, exists: bool, entry: Option<&Entry>) -> bool {
    match condition {
        TransactCondition::Exists => exists,
        TransactCondition::NotExists => !exists,
        TransactCondition::AttributeEquals { name, value } => {
            entry
                .and_then(|e| {
                    if e.is_expired() {
                        return None;
                    }
                    if name == "value" {
                        return Some(&e.value);
                    }
                    e.value.get(name)
                })
                .map_or(false, |v| v == value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_get_set_remove() {
        let store = MemoryStorage::new();
        assert!(store.get(&["a", "b"]).await.unwrap().is_none());

        store.set(&["a", "b"], json!({"x": 1}), None).await.unwrap();
        let val = store.get(&["a", "b"]).await.unwrap().unwrap();
        assert_eq!(val, json!({"x": 1}));

        store.remove(&["a", "b"]).await.unwrap();
        assert!(store.get(&["a", "b"]).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let store = MemoryStorage::new();
        let past = Utc::now() - chrono::Duration::seconds(10);
        store.set(&["expired"], json!("old"), Some(past)).await.unwrap();
        assert!(store.get(&["expired"]).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_scan() {
        let store = MemoryStorage::new();
        store.set(&["users", "1"], json!("alice"), None).await.unwrap();
        store.set(&["users", "2"], json!("bob"), None).await.unwrap();
        store.set(&["other", "x"], json!("nope"), None).await.unwrap();

        let results = store.scan(&["users"]).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_compare_and_set() {
        let store = MemoryStorage::new();

        // Set when not exists (expected=None, current=None)
        let ok = store.compare_and_set(&["k"], None, json!(1), None).await.unwrap();
        assert!(ok);

        // Fail when expected doesn't match
        let ok = store.compare_and_set(&["k"], Some(&json!(999)), json!(2), None).await.unwrap();
        assert!(!ok);

        // Succeed when expected matches
        let ok = store.compare_and_set(&["k"], Some(&json!(1)), json!(2), None).await.unwrap();
        assert!(ok);
        assert_eq!(store.get(&["k"]).await.unwrap().unwrap(), json!(2));
    }

    #[tokio::test]
    async fn test_transact() {
        let store = MemoryStorage::new();
        store.set(&["a"], json!(1), None).await.unwrap();

        store.transact(vec![
            TransactOperation::ConditionCheck {
                key: vec!["a".into()],
                condition: TransactCondition::Exists,
            },
            TransactOperation::Put {
                key: vec!["b".into()],
                value: json!(2),
                expiry: None,
            },
            TransactOperation::Delete {
                key: vec!["a".into()],
            },
        ]).await.unwrap();

        assert!(store.get(&["a"]).await.unwrap().is_none());
        assert_eq!(store.get(&["b"]).await.unwrap().unwrap(), json!(2));
    }
}
