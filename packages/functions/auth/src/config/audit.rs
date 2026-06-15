//! Audit logging configuration.

use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditLogMode {
    CloudWatch,
    None,
}

impl Default for AuditLogMode {
    fn default() -> Self {
        Self::CloudWatch
    }
}

impl FromStr for AuditLogMode {
    type Err = AuditConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "cloudwatch" => Ok(Self::CloudWatch),
            "none" => Ok(Self::None),
            other => Err(AuditConfigError::UnknownMode(other.to_string())),
        }
    }
}

#[derive(Debug, Error)]
pub enum AuditConfigError {
    #[error("unknown audit log mode `{0}`")]
    UnknownMode(String),
}
