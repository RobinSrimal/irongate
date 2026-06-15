//! Account lifecycle configuration.

use crate::store::DeletedIdentityReusePolicy;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountLifecycleConfig {
    pub deleted_identity_reuse: DeletedIdentityReusePolicy,
    pub deleted_identity_retention_days: u32,
}

impl Default for AccountLifecycleConfig {
    fn default() -> Self {
        Self {
            deleted_identity_reuse: DeletedIdentityReusePolicy::AfterRetention,
            deleted_identity_retention_days: 30,
        }
    }
}

#[derive(Debug, Error)]
pub enum AccountLifecycleConfigError {
    #[error("unknown deleted identity reuse mode `{0}`")]
    UnknownReuseMode(String),

    #[error("deleted identity retention must be between 1 and 3650 days")]
    InvalidRetention,
}

impl AccountLifecycleConfig {
    pub fn from_values(
        reuse_mode: &str,
        retention_days: u32,
    ) -> Result<Self, AccountLifecycleConfigError> {
        let deleted_identity_reuse = DeletedIdentityReusePolicy::from_str(reuse_mode)
            .map_err(AccountLifecycleConfigError::UnknownReuseMode)?;
        let config = Self {
            deleted_identity_reuse,
            deleted_identity_retention_days: retention_days,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), AccountLifecycleConfigError> {
        if self.deleted_identity_reuse == DeletedIdentityReusePolicy::AfterRetention
            && !(1..=3650).contains(&self.deleted_identity_retention_days)
        {
            return Err(AccountLifecycleConfigError::InvalidRetention);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_identity_reuse_after_retention_requires_positive_retention() {
        let lifecycle =
            AccountLifecycleConfig::from_values("after_retention", 30).expect("lifecycle config");

        assert_eq!(
            lifecycle.deleted_identity_reuse,
            DeletedIdentityReusePolicy::AfterRetention
        );
        assert!(AccountLifecycleConfig::from_values("after_retention", 0).is_err());
    }
}
