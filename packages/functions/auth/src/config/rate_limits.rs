//! Endpoint admission policies. Counts/windows are bounded before any counter operation.
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub policies: HashMap<Endpoint, RateLimitPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endpoint {
    Authorize,
    Token,
    PasswordRegister,
    PasswordVerify,
    PasswordLogin,
    PasswordResetRequest,
    PasswordResetComplete,
    OAuthRevoke,
    ProviderAuthorize,
    ProviderEmailSend,
}

impl Endpoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authorize => "authorize",
            Self::Token => "token",
            Self::PasswordRegister => "password_register",
            Self::PasswordVerify => "password_verify",
            Self::PasswordLogin => "password_login",
            Self::PasswordResetRequest => "password_reset_request",
            Self::PasswordResetComplete => "password_reset_complete",
            Self::OAuthRevoke => "oauth_revoke",
            Self::ProviderAuthorize => "provider_authorize",
            Self::ProviderEmailSend => "provider_email_send",
        }
    }

    pub(crate) fn has_secondary(self) -> bool {
        matches!(
            self,
            Self::Authorize
                | Self::Token
                | Self::OAuthRevoke
                | Self::ProviderAuthorize
                | Self::ProviderEmailSend
                | Self::PasswordRegister
                | Self::PasswordLogin
                | Self::PasswordResetRequest
        )
    }
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests: u32,
    pub window_seconds: u64,
}

impl RateLimit {
    fn valid(&self) -> bool {
        (1..=1_000_000).contains(&self.requests) && (1..=86_400).contains(&self.window_seconds)
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitPolicy {
    pub source: RateLimit,
    /// Email across sources, or client/session within a source, depending on the endpoint.
    pub secondary: Option<RateLimit>,
}

impl RateLimitPolicy {
    pub(crate) fn valid(&self, endpoint: Endpoint) -> bool {
        self.source.valid()
            && self.secondary.is_some() == endpoint.has_secondary()
            && self.secondary.as_ref().is_none_or(RateLimit::valid)
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let minute = |requests| RateLimit {
            requests,
            window_seconds: 60,
        };
        let policies = [
            (Endpoint::Authorize, 100, Some(100)),
            (Endpoint::Token, 100, Some(50)),
            (Endpoint::PasswordRegister, 60, Some(5)),
            (Endpoint::PasswordLogin, 60, Some(5)),
            (Endpoint::PasswordResetRequest, 60, Some(5)),
            (Endpoint::PasswordVerify, 5, None),
            (Endpoint::PasswordResetComplete, 5, None),
            (Endpoint::OAuthRevoke, 60, Some(20)),
            (Endpoint::ProviderAuthorize, 60, Some(20)),
            (Endpoint::ProviderEmailSend, 20, Some(3)),
        ]
        .into_iter()
        .map(|(endpoint, source, secondary)| {
            (
                endpoint,
                RateLimitPolicy {
                    source: minute(source),
                    secondary: secondary.map(minute),
                },
            )
        })
        .collect();
        Self {
            enabled: true,
            policies,
        }
    }
}
