#![allow(dead_code)]

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use irongate::email::{EmailDeliveryError, RenderedEmail, VerificationEmailSender};
use irongate::error::StorageError;
use irongate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

const SEP: &str = "\x1f";

#[derive(Clone, Default)]
pub struct NoopEmailSender;

#[async_trait]
impl VerificationEmailSender for NoopEmailSender {
    async fn send_verification_email(
        &self,
        _to: &str,
        _message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        Ok("noop-delivery".to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestStorage {
    data: Arc<RwLock<BTreeMap<String, Entry>>>,
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

impl TestStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

fn encode_key(parts: &[&str]) -> String {
    parts.join(SEP)
}

fn decode_key(encoded: &str) -> Vec<String> {
    encoded.split(SEP).map(ToString::to_string).collect()
}

#[async_trait]
impl StorageAdapter for TestStorage {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        let encoded = encode_key(key);
        let data = self
            .data
            .read()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
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
        let mut data = self
            .data
            .write()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
        data.insert(encoded, Entry { value, expiry });
        Ok(())
    }

    async fn remove(&self, key: &[&str]) -> Result<(), StorageError> {
        let encoded = encode_key(key);
        let mut data = self
            .data
            .write()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
        data.remove(&encoded);
        Ok(())
    }

    async fn scan(&self, prefix: &[&str]) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
        let prefix_str = encode_key(prefix);
        let data = self
            .data
            .read()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
        Ok(data
            .range(prefix_str.clone()..)
            .take_while(|(key, _)| key.starts_with(&prefix_str))
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(key, entry)| (decode_key(key), entry.value.clone()))
            .collect())
    }

    async fn scan_page(
        &self,
        prefix: &[&str],
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError> {
        let prefix_str = encode_key(prefix);
        let data = self
            .data
            .read()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
        let start = cursor.unwrap_or(&prefix_str);
        let items: Vec<_> = data
            .range(start.to_string()..)
            .take_while(|(key, _)| key.starts_with(&prefix_str))
            .filter(|(key, _)| cursor.map_or(true, |cursor| key.as_str() > cursor))
            .filter(|(_, entry)| !entry.is_expired())
            .take((limit + 1) as usize)
            .map(|(key, entry)| (key.clone(), decode_key(key), entry.value.clone()))
            .collect();
        let has_more = items.len() > limit as usize;
        let page = items
            .iter()
            .take(limit as usize)
            .map(|(_, key, value)| (key.clone(), value.clone()))
            .collect();
        let next_cursor = has_more
            .then(|| items.get(limit as usize - 1).map(|(key, _, _)| key.clone()))
            .flatten();
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
        let mut data = self
            .data
            .write()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
        let current = data
            .get(&encoded)
            .and_then(|entry| (!entry.is_expired()).then_some(&entry.value));
        match (expected, current) {
            (None, None) => {
                data.insert(
                    encoded,
                    Entry {
                        value: new_value,
                        expiry,
                    },
                );
                Ok(true)
            }
            (Some(expected), Some(current)) if expected == current => {
                data.insert(
                    encoded,
                    Entry {
                        value: new_value,
                        expiry,
                    },
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        let mut data = self
            .data
            .write()
            .map_err(|err| StorageError::DynamoDB(format!("lock poisoned: {err}")))?;
        for op in &operations {
            match op {
                TransactOperation::ConditionCheck { key, condition } => {
                    let encoded = key.join(SEP);
                    let exists = data
                        .get(&encoded)
                        .map_or(false, |entry| !entry.is_expired());
                    if !check_condition(condition, exists, data.get(&encoded)) {
                        return Err(StorageError::ConditionFailed(
                            "transaction condition failed".into(),
                        ));
                    }
                }
                TransactOperation::Update {
                    key,
                    condition: Some(condition),
                    ..
                } => {
                    let encoded = key.join(SEP);
                    let exists = data
                        .get(&encoded)
                        .map_or(false, |entry| !entry.is_expired());
                    if !check_condition(condition, exists, data.get(&encoded)) {
                        return Err(StorageError::ConditionFailed(
                            "transaction condition failed".into(),
                        ));
                    }
                }
                _ => {}
            }
        }
        for op in operations {
            match op {
                TransactOperation::Put { key, value, expiry } => {
                    data.insert(key.join(SEP), Entry { value, expiry });
                }
                TransactOperation::Delete { key } => {
                    data.remove(&key.join(SEP));
                }
                TransactOperation::Update { key, updates, .. } => {
                    if let Some(entry) = data.get_mut(&key.join(SEP)) {
                        if let (Value::Object(existing), Value::Object(updates)) =
                            (&mut entry.value, updates)
                        {
                            for (field, value) in updates {
                                existing.insert(field, value);
                            }
                        }
                    }
                }
                TransactOperation::ConditionCheck { .. } => {}
            }
        }
        Ok(())
    }
}

fn check_condition(condition: &TransactCondition, exists: bool, entry: Option<&Entry>) -> bool {
    match condition {
        TransactCondition::Exists => exists,
        TransactCondition::NotExists => !exists,
        TransactCondition::AttributeEquals { name, value } => entry
            .and_then(|entry| {
                if entry.is_expired() {
                    return None;
                }
                if name == "value" {
                    return Some(&entry.value);
                }
                entry.value.get(name)
            })
            .map_or(false, |current| current == value),
    }
}
