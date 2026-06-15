//! Storage layer for Irongate.
//!
//! Provides the `StorageAdapter` trait and DynamoDB implementation.

mod adapter;
mod dynamo;
#[cfg(test)]
pub(crate) mod test_support;

pub use adapter::{StorageAdapter, TransactCondition, TransactOperation};
pub use dynamo::DynamoStorage;
