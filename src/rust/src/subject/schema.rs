//! Subject schema and validation.
//!
//! Defines subject types and their properties.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Subject definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    /// Subject type (e.g., "user", "account")
    #[serde(rename = "type")]
    pub subject_type: String,
    /// Subject properties
    pub properties: serde_json::Value,
}

impl Subject {
    /// Create a new subject
    pub fn new(subject_type: impl Into<String>, properties: serde_json::Value) -> Self {
        Self {
            subject_type: subject_type.into(),
            properties,
        }
    }

    /// Generate the subject ID (type:hash)
    pub fn id(&self) -> String {
        let hash = self.hash();
        format!("{}:{}", self.subject_type, hash)
    }

    /// Generate the subject hash
    ///
    /// Uses SHA-256 (not truncated) for security.
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();

        // Hash the subject type and properties
        hasher.update(self.subject_type.as_bytes());
        hasher.update(b":");

        // Serialize properties deterministically
        let props = serde_json::to_string(&self.properties).unwrap_or_default();
        hasher.update(props.as_bytes());

        let result = hasher.finalize();
        hex::encode(result)
    }
}

/// Validate subject properties against a schema
pub fn validate_subject(subject: &Subject, schema: &SubjectSchema) -> Result<(), String> {
    // TODO: Implement schema validation
    Ok(())
}

/// Subject schema definition
#[derive(Debug, Clone)]
pub struct SubjectSchema {
    pub subject_type: String,
    pub required_properties: Vec<String>,
    pub optional_properties: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_subject_id() {
        let subject = Subject::new("user", json!({"email": "test@example.com"}));
        let id = subject.id();

        assert!(id.starts_with("user:"));
        assert_eq!(id.len(), 5 + 64); // "user:" + 64 hex chars
    }

    #[test]
    fn test_subject_hash_consistency() {
        let subject1 = Subject::new("user", json!({"email": "test@example.com"}));
        let subject2 = Subject::new("user", json!({"email": "test@example.com"}));

        assert_eq!(subject1.hash(), subject2.hash());
    }

    #[test]
    fn test_different_subjects_different_hashes() {
        let subject1 = Subject::new("user", json!({"email": "test1@example.com"}));
        let subject2 = Subject::new("user", json!({"email": "test2@example.com"}));

        assert_ne!(subject1.hash(), subject2.hash());
    }
}
