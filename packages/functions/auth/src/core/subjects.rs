//! Opaque subject identifiers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A persisted account subject.
///
/// Subjects are generated and stored. They are never derived from email,
/// provider IDs, or other reusable identity attributes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Subject(String);

impl Subject {
    pub fn generate() -> Self {
        Self(format!("user_{}", Uuid::new_v4().simple()))
    }

    pub fn from_persisted(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
