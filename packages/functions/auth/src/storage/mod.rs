//! Storage layer for Irongate.
//!
//! Provides the internal storage trait and DynamoDB implementation.
//!
//! `StorageAdapter` remains public so integration tests can provide an
//! in-memory backend. Runtime route, provider, and admin code must not import
//! it; `scripts/validate-store-boundary.mjs` enforces that boundary.

mod adapter;
mod dynamo;
#[cfg(test)]
pub(crate) mod test_support;

pub use adapter::{StorageAdapter, TransactCondition, TransactOperation};
pub use dynamo::DynamoStorage;
