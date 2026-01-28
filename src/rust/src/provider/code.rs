//! Code provider implementation.
//!
//! OTP-based authentication (email or SMS codes).

use super::traits::{Provider, ProviderContext};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

/// Code provider configuration
#[derive(Debug, Clone)]
pub struct CodeConfig {
    /// Code length
    pub length: usize,
    /// Code expiry in seconds
    pub expiry: u64,
    /// Callback for sending the code
    pub send_code: Option<fn(destination: &str, code: &str) -> Result<(), String>>,
}

impl Default for CodeConfig {
    fn default() -> Self {
        Self {
            length: 6,
            expiry: 600, // 10 minutes
            send_code: None,
        }
    }
}

/// Code provider (OTP)
pub struct CodeProvider {
    pub config: CodeConfig,
}

impl CodeProvider {
    pub fn new(config: CodeConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Provider for CodeProvider {
    fn name(&self) -> &str {
        "code"
    }

    fn provider_type(&self) -> &str {
        "code"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        _ctx: ProviderContext<S>,
    ) -> Router {
        // TODO: Add code-specific routes:
        // - POST /code/request
        // - POST /code/verify
        router
    }
}
