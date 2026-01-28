//! Password provider implementation.
//!
//! Email/password authentication with verification codes.

use super::traits::{Provider, ProviderContext, SubjectInfo};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

/// Password provider configuration
#[derive(Debug, Clone)]
pub struct PasswordConfig {
    /// Whether email verification is required
    pub require_verification: bool,
    /// Code length for verification emails
    pub code_length: usize,
    /// Code expiry in seconds
    pub code_expiry: u64,
}

impl Default for PasswordConfig {
    fn default() -> Self {
        Self {
            require_verification: true,
            code_length: 6,
            code_expiry: 600, // 10 minutes
        }
    }
}

/// Password provider
pub struct PasswordProvider {
    pub config: PasswordConfig,
}

impl PasswordProvider {
    pub fn new(config: PasswordConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Provider for PasswordProvider {
    fn name(&self) -> &str {
        "password"
    }

    fn provider_type(&self) -> &str {
        "password"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        _ctx: ProviderContext<S>,
    ) -> Router {
        // TODO: Add password-specific routes:
        // - POST /password/register
        // - POST /password/login
        // - POST /password/verify
        // - POST /password/change
        // - POST /password/forgot
        router
    }
}
