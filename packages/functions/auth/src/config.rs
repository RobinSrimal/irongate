//! Configuration for Irongate OAuth 2.0 server.
//!
//! All configuration is loaded from environment variables with secure defaults.

use std::collections::HashMap;
use std::sync::Arc;

use crate::store::AuthStore;

pub mod account_lifecycle;
pub mod apple;
pub mod audit;
pub mod client_file;
pub mod email;
pub mod environment;
pub mod google;
pub mod signing;
pub mod ttls;

/// Main configuration for the Irongate server
#[derive(Debug, Clone)]
pub struct Config {
    /// DynamoDB table name
    pub table_name: String,

    /// Base URL for the issuer (JWT `iss` claim)
    pub issuer_url: Option<String>,

    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,

    /// Token TTL configuration
    pub tokens: TokenConfig,

    /// Development mode (allows localhost redirects)
    pub dev_mode: bool,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let dev_mode = std::env::var("DEV_MODE")
            .map(|v| v == "true")
            .unwrap_or(false);

        let issuer_url = if dev_mode {
            std::env::var("ISSUER_URL").ok()
        } else {
            Some(
                std::env::var("ISSUER_URL")
                    .expect("ISSUER_URL environment variable required in production"),
            )
        };

        Self {
            table_name: std::env::var("DYNAMODB_TABLE")
                .expect("DYNAMODB_TABLE environment variable required"),
            issuer_url,
            rate_limit: RateLimitConfig::default(),
            tokens: TokenConfig::default(),
            dev_mode,
        }
    }

    /// Create configuration for local development (no DynamoDB required)
    pub fn dev() -> Self {
        Self {
            table_name: "local-dev".to_string(),
            issuer_url: std::env::var("ISSUER_URL").ok(),
            rate_limit: RateLimitConfig::default(),
            tokens: TokenConfig::default(),
            dev_mode: true,
        }
    }

    #[cfg(test)]
    pub fn from_env_for_test(dev_mode: bool) -> Self {
        if dev_mode {
            std::env::set_var("DEV_MODE", "true");
        } else {
            std::env::remove_var("DEV_MODE");
        }
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static::lazy_static;
    use std::sync::Mutex;

    lazy_static! {
        static ref ENV_LOCK: Mutex<()> = Mutex::new(());
    }

    #[test]
    fn issuer_required_in_production() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev_issuer = std::env::var("ISSUER_URL").ok();
        let prev_dev = std::env::var("DEV_MODE").ok();
        let prev_table = std::env::var("DYNAMODB_TABLE").ok();

        std::env::remove_var("ISSUER_URL");
        std::env::remove_var("DEV_MODE");
        std::env::set_var("DYNAMODB_TABLE", "test-table");

        let result = std::panic::catch_unwind(|| {
            let _ = Config::from_env();
        });

        assert!(result.is_err(), "expected panic when ISSUER_URL missing");

        if let Some(val) = prev_issuer {
            std::env::set_var("ISSUER_URL", val);
        } else {
            std::env::remove_var("ISSUER_URL");
        }
        if let Some(val) = prev_dev {
            std::env::set_var("DEV_MODE", val);
        } else {
            std::env::remove_var("DEV_MODE");
        }
        if let Some(val) = prev_table {
            std::env::set_var("DYNAMODB_TABLE", val);
        } else {
            std::env::remove_var("DYNAMODB_TABLE");
        }
    }

    #[test]
    fn issuer_optional_in_dev_mode() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev_issuer = std::env::var("ISSUER_URL").ok();
        let prev_dev = std::env::var("DEV_MODE").ok();
        let prev_table = std::env::var("DYNAMODB_TABLE").ok();

        std::env::remove_var("ISSUER_URL");
        std::env::set_var("DEV_MODE", "true");
        std::env::set_var("DYNAMODB_TABLE", "test-table");

        let cfg = Config::from_env();
        assert!(cfg.issuer_url.is_none());
        assert!(cfg.dev_mode);

        if let Some(val) = prev_issuer {
            std::env::set_var("ISSUER_URL", val);
        } else {
            std::env::remove_var("ISSUER_URL");
        }
        if let Some(val) = prev_dev {
            std::env::set_var("DEV_MODE", val);
        } else {
            std::env::remove_var("DEV_MODE");
        }
        if let Some(val) = prev_table {
            std::env::set_var("DYNAMODB_TABLE", val);
        } else {
            std::env::remove_var("DYNAMODB_TABLE");
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled
    pub enabled: bool,
    /// Limits per endpoint
    pub limits: HashMap<Endpoint, RateLimit>,
}

/// Endpoint identifier for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endpoint {
    Authorize,
    Token,
    PasswordRegister,
    PasswordVerify,
    PasswordLogin,
    PasswordResetRequest,
    PasswordResetComplete,
    CodeVerify,
    AdminApi,
}

impl Endpoint {
    /// Get the string representation for storage keys
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authorize => "authorize",
            Self::Token => "token",
            Self::PasswordRegister => "password_register",
            Self::PasswordVerify => "password_verify",
            Self::PasswordLogin => "password_login",
            Self::PasswordResetRequest => "password_reset_request",
            Self::PasswordResetComplete => "password_reset_complete",
            Self::CodeVerify => "code_verify",
            Self::AdminApi => "admin_api",
        }
    }
}

/// Rate limit definition
#[derive(Debug, Clone)]
pub struct RateLimit {
    /// Maximum requests allowed
    pub requests: u32,
    /// Time window in seconds
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut limits = HashMap::new();

        limits.insert(
            Endpoint::Authorize,
            RateLimit {
                requests: 100,
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::Token,
            RateLimit {
                requests: 50,
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::PasswordRegister,
            RateLimit {
                requests: 5, // Very aggressive for password endpoints
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::PasswordVerify,
            RateLimit {
                requests: 5, // Very aggressive for password endpoints
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::PasswordLogin,
            RateLimit {
                requests: 5,
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::PasswordResetRequest,
            RateLimit {
                requests: 5,
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::PasswordResetComplete,
            RateLimit {
                requests: 5,
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::CodeVerify,
            RateLimit {
                requests: 5,
                window_seconds: 60,
            },
        );
        limits.insert(
            Endpoint::AdminApi,
            RateLimit {
                requests: 100,
                window_seconds: 60,
            },
        );

        Self {
            enabled: true,
            limits,
        }
    }
}

/// Token TTL configuration
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// Access token TTL in seconds (default: 30 days)
    pub access_token_ttl: u64,
    /// Refresh token TTL in seconds (default: 1 year)
    pub refresh_token_ttl: u64,
    /// Refresh token reuse window in seconds (default: 60s)
    pub refresh_reuse_window: u64,
    /// Authorization code TTL in seconds (default: 60s)
    pub code_ttl: u64,
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            access_token_ttl: 60 * 60 * 24 * 30,   // 30 days
            refresh_token_ttl: 60 * 60 * 24 * 365, // 1 year
            refresh_reuse_window: 60,              // 60 seconds
            code_ttl: 60,                          // 60 seconds
        }
    }
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub store: AuthStore,
    pub config: Arc<Config>,
    pub runtime: Arc<environment::RuntimeAuthConfig>,
    pub email_sender: Arc<dyn crate::email::VerificationEmailSender>,
    pub google_client: Arc<dyn crate::providers::google::GoogleOidcClient>,
    pub apple_client: Arc<dyn crate::providers::apple::AppleOidcClient>,
}
