//! Storage layer for Irongate.
//!
//! Provides the `StorageAdapter` trait and DynamoDB implementation.

mod adapter;
mod dynamo;

pub use adapter::StorageAdapter;
pub use dynamo::DynamoStorage;
