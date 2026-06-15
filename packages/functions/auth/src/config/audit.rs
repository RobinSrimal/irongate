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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_mode_accepts_cloudwatch_and_rejects_unknown_modes() {
        assert_eq!(
            AuditLogMode::from_str("cloudwatch").expect("audit mode"),
            AuditLogMode::CloudWatch
        );
        assert!(AuditLogMode::from_str("s3").is_err());
    }
}
