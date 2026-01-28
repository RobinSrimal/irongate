//! Configuration for Irongate OAuth 2.0 server.
//!
//! All configuration is loaded from environment variables with secure defaults.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

/// Main configuration for the Irongate server
#[derive(Debug, Clone)]
pub struct Config {
    /// DynamoDB table name
    pub table_name: String,

    /// Base URL for the issuer (JWT `iss` claim)
    pub issuer_url: Option<String>,

    /// Proxy configuration for header trust
    pub proxy: ProxyConfig,

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
        Self {
            table_name: std::env::var("DYNAMODB_TABLE")
                .expect("DYNAMODB_TABLE environment variable required"),
            issuer_url: std::env::var("ISSUER_URL").ok(),
            proxy: ProxyConfig::from_env(),
            rate_limit: RateLimitConfig::default(),
            tokens: TokenConfig::default(),
            dev_mode: std::env::var("DEV_MODE")
                .map(|v| v == "true")
                .unwrap_or(false),
        }
    }
}

/// Proxy trust configuration
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Which proxies to trust for X-Forwarded-* headers
    pub trusted_proxies: TrustedProxies,
}

/// Trusted proxy configuration
#[derive(Debug, Clone)]
pub enum TrustedProxies {
    /// Don't trust any proxy headers (safest)
    None,
    /// Trust API Gateway headers only
    ApiGateway,
    /// Trust specific IP ranges
    IpRanges(Vec<IpRange>),
}

/// IP range for trusted proxy configuration
#[derive(Debug, Clone)]
pub struct IpRange {
    pub network: IpAddr,
    pub prefix_len: u8,
}

impl ProxyConfig {
    /// Load proxy configuration from environment
    pub fn from_env() -> Self {
        let trusted = std::env::var("TRUSTED_PROXIES").unwrap_or_else(|_| "none".to_string());

        Self {
            trusted_proxies: match trusted.as_str() {
                "none" => TrustedProxies::None,
                "api-gateway" => TrustedProxies::ApiGateway,
                _ranges => {
                    // TODO: Parse CIDR ranges
                    TrustedProxies::None
                }
            },
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
    PasswordLogin,
    CodeVerify,
    AdminApi,
}

impl Endpoint {
    /// Get the string representation for storage keys
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authorize => "authorize",
            Self::Token => "token",
            Self::PasswordLogin => "password_login",
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
            Endpoint::PasswordLogin,
            RateLimit {
                requests: 5, // Very aggressive for password endpoints
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
            access_token_ttl: 60 * 60 * 24 * 30,      // 30 days
            refresh_token_ttl: 60 * 60 * 24 * 365,   // 1 year
            refresh_reuse_window: 60,                 // 60 seconds
            code_ttl: 60,                             // 60 seconds
        }
    }
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState<S: crate::storage::StorageAdapter> {
    pub storage: Arc<S>,
    pub config: Arc<Config>,
}
