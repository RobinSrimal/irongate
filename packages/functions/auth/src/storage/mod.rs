//! Storage layer for Irongate.
//!
//! Provides the `StorageAdapter` trait and DynamoDB implementation.

mod adapter;
mod dynamo;
pub mod memory;

pub use adapter::{StorageAdapter, TransactCondition, TransactOperation};
pub use dynamo::DynamoStorage;
pub use memory::MemoryStorage;
