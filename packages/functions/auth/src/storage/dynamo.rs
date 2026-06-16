//! DynamoDB storage implementation.
//!
//! Implements the `StorageAdapter` trait for AWS DynamoDB.

use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

use super::adapter::{StorageAdapter, TransactCondition, TransactOperation};
use crate::error::StorageError;

/// Apply a condition expression to a ConditionCheck builder
fn apply_condition(
    mut builder: aws_sdk_dynamodb::types::builders::ConditionCheckBuilder,
    condition: &TransactCondition,
) -> aws_sdk_dynamodb::types::builders::ConditionCheckBuilder {
    match condition {
        TransactCondition::Exists => {
            builder = builder.condition_expression("attribute_exists(pk)");
        }
        TransactCondition::NotExists => {
            builder = builder.condition_expression("attribute_not_exists(pk)");
        }
        TransactCondition::AttributeEquals { name, value } => {
            let val_str = serde_json::to_string(value).unwrap_or_default();
            builder = builder
                .condition_expression("#attr = :val")
                .expression_attribute_names("#attr", name)
                .expression_attribute_values(
                    ":val",
                    aws_sdk_dynamodb::types::AttributeValue::S(val_str),
                );
        }
    }
    builder
}

/// Apply a condition expression to an Update builder
fn apply_update_condition(
    mut builder: aws_sdk_dynamodb::types::builders::UpdateBuilder,
    condition: &TransactCondition,
) -> aws_sdk_dynamodb::types::builders::UpdateBuilder {
    match condition {
        TransactCondition::Exists => {
            builder = builder.condition_expression("attribute_exists(pk)");
        }
        TransactCondition::NotExists => {
            builder = builder.condition_expression("attribute_not_exists(pk)");
        }
        TransactCondition::AttributeEquals { name, value } => {
            let val_str = serde_json::to_string(value).unwrap_or_default();
            builder = builder
                .condition_expression("#cond_attr = :cond_val")
                .expression_attribute_names("#cond_attr", name)
                .expression_attribute_values(
                    ":cond_val",
                    aws_sdk_dynamodb::types::AttributeValue::S(val_str),
                );
        }
    }
    builder
}

/// Apply a condition expression to a Put builder
fn apply_put_condition(
    mut builder: aws_sdk_dynamodb::types::builders::PutBuilder,
    condition: &TransactCondition,
) -> aws_sdk_dynamodb::types::builders::PutBuilder {
    match condition {
        TransactCondition::Exists => {
            builder = builder.condition_expression("attribute_exists(pk)");
        }
        TransactCondition::NotExists => {
            builder = builder.condition_expression("attribute_not_exists(pk)");
        }
        TransactCondition::AttributeEquals { name, value } => {
            let val_str = serde_json::to_string(value).unwrap_or_default();
            builder = builder
                .condition_expression("#cond_attr = :cond_val")
                .expression_attribute_names("#cond_attr", name)
                .expression_attribute_values(
                    ":cond_val",
                    aws_sdk_dynamodb::types::AttributeValue::S(val_str),
                );
        }
    }
    builder
}

/// Apply a condition expression to a Delete builder
fn apply_delete_condition(
    mut builder: aws_sdk_dynamodb::types::builders::DeleteBuilder,
    condition: &TransactCondition,
) -> aws_sdk_dynamodb::types::builders::DeleteBuilder {
    match condition {
        TransactCondition::Exists => {
            builder = builder.condition_expression("attribute_exists(pk)");
        }
        TransactCondition::NotExists => {
            builder = builder.condition_expression("attribute_not_exists(pk)");
        }
        TransactCondition::AttributeEquals { name, value } => {
            let val_str = serde_json::to_string(value).unwrap_or_default();
            builder = builder
                .condition_expression("#cond_attr = :cond_val")
                .expression_attribute_names("#cond_attr", name)
                .expression_attribute_values(
                    ":cond_val",
                    aws_sdk_dynamodb::types::AttributeValue::S(val_str),
                );
        }
    }
    builder
}

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

    async fn query_prefix(
        &self,
        prefix: &[&str],
    ) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
        let (pk, sk_prefix) = Self::encode_key(prefix);
        let now = Utc::now().timestamp();

        let mut builder = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression(if sk_prefix.is_empty() {
                "pk = :pk".to_string()
            } else {
                "pk = :pk AND begins_with(sk, :sk)".to_string()
            })
            .expression_attribute_values(":pk", aws_sdk_dynamodb::types::AttributeValue::S(pk));

        if !sk_prefix.is_empty() {
            builder = builder.expression_attribute_values(
                ":sk",
                aws_sdk_dynamodb::types::AttributeValue::S(sk_prefix),
            );
        }

        let mut results = Vec::new();
        let mut exclusive_start_key = None;

        loop {
            let mut request = builder.clone();
            if let Some(start_key) = exclusive_start_key.take() {
                request = request.set_exclusive_start_key(Some(start_key));
            }

            let response = request
                .send()
                .await
                .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

            if let Some(items) = response.items {
                for item in items {
                    // Check TTL
                    if let Some(aws_sdk_dynamodb::types::AttributeValue::N(expiry_str)) =
                        item.get("expiry")
                    {
                        if let Ok(expiry) = expiry_str.parse::<i64>() {
                            if expiry < now {
                                continue;
                            }
                        }
                    }

                    // Extract key parts
                    let item_pk = match item.get("pk") {
                        Some(aws_sdk_dynamodb::types::AttributeValue::S(s)) => s.as_str(),
                        _ => continue,
                    };
                    let item_sk = match item.get("sk") {
                        Some(aws_sdk_dynamodb::types::AttributeValue::S(s)) => s.as_str(),
                        _ => "",
                    };
                    let key_parts = Self::decode_key(item_pk, item_sk);

                    // Extract value
                    if let Some(aws_sdk_dynamodb::types::AttributeValue::S(value_str)) =
                        item.get("value")
                    {
                        let value: Value = serde_json::from_str(value_str)
                            .map_err(|e| StorageError::DynamoDB(e.to_string()))?;
                        results.push((key_parts, value));
                    }
                }
            }

            match response.last_evaluated_key {
                Some(key) => exclusive_start_key = Some(key),
                None => break,
            }
        }

        Ok(results)
    }

    async fn query_prefix_page(
        &self,
        prefix: &[&str],
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError> {
        let (pk, sk_prefix) = Self::encode_key(prefix);
        let now = Utc::now().timestamp();

        let mut request = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression(if sk_prefix.is_empty() {
                "pk = :pk".to_string()
            } else {
                "pk = :pk AND begins_with(sk, :sk)".to_string()
            })
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .limit(limit as i32);

        if !sk_prefix.is_empty() {
            request = request.expression_attribute_values(":sk", AttributeValue::S(sk_prefix));
        }

        // Decode cursor into ExclusiveStartKey
        if let Some(cursor_str) = cursor {
            let decoded = URL_SAFE_NO_PAD
                .decode(cursor_str)
                .map_err(|e| StorageError::DynamoDB(format!("Invalid cursor: {}", e)))?;
            let start_key: HashMap<String, String> = serde_json::from_slice(&decoded)
                .map_err(|e| StorageError::DynamoDB(format!("Invalid cursor: {}", e)))?;
            let mut key_map = HashMap::new();
            for (k, v) in start_key {
                key_map.insert(k, AttributeValue::S(v));
            }
            request = request.set_exclusive_start_key(Some(key_map));
        }

        let response = request
            .send()
            .await
            .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

        let mut results = Vec::new();
        if let Some(items) = response.items {
            for item in items {
                // Check TTL
                if let Some(AttributeValue::N(expiry_str)) = item.get("expiry") {
                    if let Ok(expiry) = expiry_str.parse::<i64>() {
                        if expiry < now {
                            continue;
                        }
                    }
                }

                let item_pk = match item.get("pk") {
                    Some(AttributeValue::S(s)) => s.as_str(),
                    _ => continue,
                };
                let item_sk = match item.get("sk") {
                    Some(AttributeValue::S(s)) => s.as_str(),
                    _ => "",
                };
                let key_parts = Self::decode_key(item_pk, item_sk);

                if let Some(AttributeValue::S(value_str)) = item.get("value") {
                    let value: Value = serde_json::from_str(value_str)
                        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;
                    results.push((key_parts, value));
                }
            }
        }

        // Encode LastEvaluatedKey as cursor
        let next_cursor = response.last_evaluated_key.map(|lek| {
            let simple: HashMap<String, String> = lek
                .into_iter()
                .filter_map(|(k, v)| match v {
                    AttributeValue::S(s) => Some((k, s)),
                    _ => None,
                })
                .collect();
            let json = serde_json::to_vec(&simple).unwrap_or_default();
            URL_SAFE_NO_PAD.encode(json)
        });

        Ok((results, next_cursor))
    }

    async fn compare_and_set(
        &self,
        key: &[&str],
        expected: Option<&Value>,
        new_value: Value,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        let (pk, sk) = Self::encode_key(key);
        let value_str =
            serde_json::to_string(&new_value).map_err(|e| StorageError::DynamoDB(e.to_string()))?;

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

        match expected {
            None => {
                // Item must not exist
                request = request.condition_expression("attribute_not_exists(pk)")
            }
            Some(expected_val) => {
                // Item must exist with the expected value
                let expected_str = serde_json::to_string(expected_val)
                    .map_err(|e| StorageError::DynamoDB(e.to_string()))?;
                request = request
                    .condition_expression("#v = :expected")
                    .expression_attribute_names("#v", "value")
                    .expression_attribute_values(
                        ":expected",
                        aws_sdk_dynamodb::types::AttributeValue::S(expected_str),
                    );
            }
        }

        match request.send().await {
            Ok(_) => Ok(true),
            Err(e) => {
                let service_err = e.into_service_error();
                if service_err.is_conditional_check_failed_exception() {
                    Ok(false)
                } else {
                    Err(StorageError::DynamoDB(service_err.to_string()))
                }
            }
        }
    }

    async fn transact(&self, operations: Vec<TransactOperation>) -> Result<(), StorageError> {
        use aws_sdk_dynamodb::types::{
            AttributeValue, ConditionCheck, Delete, Put, TransactWriteItem, Update,
        };

        let mut items: Vec<TransactWriteItem> = Vec::with_capacity(operations.len());

        for op in operations {
            match op {
                TransactOperation::Put {
                    key,
                    value,
                    expiry,
                    condition,
                } => {
                    let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                    let (pk, sk) = Self::encode_key(&key_refs);
                    let value_str = serde_json::to_string(&value)
                        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

                    let mut put = Put::builder()
                        .table_name(&self.table_name)
                        .item("pk", AttributeValue::S(pk))
                        .item("sk", AttributeValue::S(sk))
                        .item("value", AttributeValue::S(value_str));

                    if let Some(exp) = expiry {
                        put = put.item("expiry", AttributeValue::N(exp.timestamp().to_string()));
                    }
                    if let Some(condition) = &condition {
                        put = apply_put_condition(put, condition);
                    }

                    items.push(
                        TransactWriteItem::builder()
                            .put(
                                put.build()
                                    .map_err(|e| StorageError::DynamoDB(e.to_string()))?,
                            )
                            .build(),
                    );
                }
                TransactOperation::Delete { key, condition } => {
                    let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                    let (pk, sk) = Self::encode_key(&key_refs);

                    let mut delete = Delete::builder()
                        .table_name(&self.table_name)
                        .key("pk", AttributeValue::S(pk))
                        .key("sk", AttributeValue::S(sk));

                    if let Some(condition) = &condition {
                        delete = apply_delete_condition(delete, condition);
                    }

                    let delete = delete
                        .build()
                        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

                    items.push(TransactWriteItem::builder().delete(delete).build());
                }
                TransactOperation::ConditionCheck { key, condition } => {
                    let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                    let (pk, sk) = Self::encode_key(&key_refs);

                    let mut check = ConditionCheck::builder()
                        .table_name(&self.table_name)
                        .key("pk", AttributeValue::S(pk))
                        .key("sk", AttributeValue::S(sk));

                    check = apply_condition(check, &condition);

                    items.push(
                        TransactWriteItem::builder()
                            .condition_check(
                                check
                                    .build()
                                    .map_err(|e| StorageError::DynamoDB(e.to_string()))?,
                            )
                            .build(),
                    );
                }
                TransactOperation::Update {
                    key,
                    updates,
                    condition,
                } => {
                    let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                    let (pk, sk) = Self::encode_key(&key_refs);
                    let value_str = serde_json::to_string(&updates)
                        .map_err(|e| StorageError::DynamoDB(e.to_string()))?;

                    let mut update = Update::builder()
                        .table_name(&self.table_name)
                        .key("pk", AttributeValue::S(pk))
                        .key("sk", AttributeValue::S(sk))
                        .update_expression("SET #v = :val")
                        .expression_attribute_names("#v", "value")
                        .expression_attribute_values(":val", AttributeValue::S(value_str));

                    if let Some(cond) = &condition {
                        update = apply_update_condition(update, cond);
                    }

                    items.push(
                        TransactWriteItem::builder()
                            .update(
                                update
                                    .build()
                                    .map_err(|e| StorageError::DynamoDB(e.to_string()))?,
                            )
                            .build(),
                    );
                }
            }
        }

        self.client
            .transact_write_items()
            .set_transact_items(Some(items))
            .send()
            .await
            .map_err(|e| {
                let service_err = e.into_service_error();
                if service_err.is_transaction_canceled_exception() {
                    StorageError::TransactionConflict
                } else {
                    StorageError::DynamoDB(service_err.to_string())
                }
            })?;

        Ok(())
    }
}
