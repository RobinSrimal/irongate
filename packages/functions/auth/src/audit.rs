//! Audit logging utilities.
//!
//! Records security-relevant events for later review.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::crypto::random::generate_uuid;
use crate::storage::StorageAdapter;

/// Audit event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AuditEvent {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            id: generate_uuid(),
            event_type: event_type.into(),
            timestamp: Utc::now(),
            client_id: None,
            subject: None,
            token_hash: None,
            ip: None,
            detail: None,
        }
    }
}

/// Persist an audit event.
pub async fn record_event<S: StorageAdapter + ?Sized>(
    storage: &S,
    event: AuditEvent,
) -> Result<(), String> {
    let value = serde_json::to_value(&event).map_err(|e| e.to_string())?;
    storage
        .set(&["audit", &event.id], value, None)
        .await
        .map_err(|e| e.to_string())
}
