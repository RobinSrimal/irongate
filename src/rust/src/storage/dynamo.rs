//! DynamoDB storage implementation.
//!
//! Implements the `StorageAdapter` trait for AWS DynamoDB.

use async_trait::async_trait;
use aws_sdk_dynamodb::Client;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::adapter::{StorageAdapter, TransactCondition, TransactOperation};
use crate::error::StorageError;

/// Key separator for encoding multi-part keys
const KEY_SEPARATOR: char = '\x1f'; // Unit Separator (ASCII 31)

/// DynamoDB storage adapter
#[derive(Clone)]
pub struct DynamoStorage {
    client: Client,
    table_name: String,
}

impl DynamoStorage {
    /// Create a new DynamoDB storage adapter
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    /// Encode a key array into (pk, sk) pair
    fn encode_key(key: &[&str]) -> (String, String) {
        match key.len() {
            0 => panic!("Key must have at least one part"),
            1 => (key[0].to_string(), String::new()),
            2 => (key[0].to_string(), key[1].to_string()),
            _ => {
                // For keys with >2 parts: pk = join(key[0..2]), sk = join(key[2..])
                let pk = format!("{}{}{}", key[0], KEY_SEPARATOR, key[1]);
                let sk = key[2..].join(&KEY_SEPARATOR.to_string());
                (pk, sk)
            }
        }
    }

    /// Decode (pk, sk) back into key parts
    fn decode_key(pk: &str, sk: &str) -> Vec<String> {
        let mut parts: Vec<String> = pk.split(KEY_SEPARATOR).map(String::from).collect();
        if !sk.is_empty() {
            parts.extend(sk.split(KEY_SEPARATOR).map(String::from));
        }
        parts
    }
}

#[async_trait]
impl StorageAdapter for DynamoStorage {
    async fn get(&self, key: &[&str]) -> Result<Option<Value>, StorageError> {
        let (pk, sk) = Self::encode_key(key);

        let result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("pk", aws_sdk_dynamodb::types::AttributeValue::S(pk))
            .key("sk", aws_sdk_dynamodb::types::AttributeValue::S(sk))
            .send()
            .await
            .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

        match result.item {
            None => Ok(None),
            Some(item) => {
                // Check TTL
                if let Some(expiry_attr) = item.get("expiry") {
                    if let aws_sdk_dynamodb::types::AttributeValue::N(expiry_str) = expiry_attr {
                        if let Ok(expiry) = expiry_str.parse::<i64>() {
                            if expiry < Utc::now().timestamp() {
                                return Ok(None);
                            }
                        }
                    }
                }

                // Extract value
                if let Some(value_attr) = item.get("value") {
                    if let aws_sdk_dynamodb::types::AttributeValue::S(value_str) = value_attr {
                        let value: Value = serde_json::from_str(value_str)
                            .map_err(|e| StorageError::DynamoDB(e.to_string()))?;
                        return Ok(Some(value));
                    }
                }

                Ok(None)
            }
        }
    }

    async fn set(
        &self,
        key: &[&str],
        value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError> {
        let (pk, sk) = Self::encode_key(key);
        let value_str =
            serde_json::to_string(&value).map_err(|e| StorageError::DynamoDB(e.to_string()))?;

        let mut request = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("pk", aws_sdk_dynamodb::types::AttributeValue::S(pk))
            .item("sk", aws_sdk_dynamodb::types::AttributeValue::S(sk))
            .item(
                "value",
                aws_sdk_dynamodb::types::AttributeValue::S(value_str),
            );

        if let Some(exp) = expiry {
            request = request.item(
                "expiry",
                aws_sdk_dynamodb::types::AttributeValue::N(exp.timestamp().to_string()),
            );
        }

        request
            .send()
            .await
            .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

        Ok(())
    }

    async fn remove(&self, key: &[&str]) -> Result<(), StorageError> {
        let (pk, sk) = Self::encode_key(key);

        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key("pk", aws_sdk_dynamodb::types::AttributeValue::S(pk))
            .key("sk", aws_sdk_dynamodb::types::AttributeValue::S(sk))
            .send()
            .await
            .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

        Ok(())
    }

    async fn scan(&self, prefix: &[&str]) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
        todo!("Implement scan for DynamoDB")
    }

    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        new_value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        todo!("Implement compare_and_set for DynamoDB")
    }

    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        todo!("Implement transact for DynamoDB")
    }
}
