//! HMAC lookup digests for bearer values and reusable identity attributes.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Logical lookup families are domain-separated before HMAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupFamily {
    AuthorizeSession,
    ProviderState,
    SignInConfirmation,
    SignInBrowser,
    SignInConfirmationCode,
    AuthorizationCode,
    RefreshToken,
    EmailVerification,
    PasswordReset,
    Email,
    PasswordIdentity,
    GoogleIdentity,
    AppleIdentity,
    RateLimitSource,
    RateLimitEmail,
    RateLimitClientSource,
    RateLimitProviderSource,
}

impl LookupFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::AuthorizeSession => "oauth_session",
            Self::ProviderState => "provider_state",
            Self::SignInConfirmation => "sign_in_confirmation",
            Self::SignInBrowser => "sign_in_browser",
            Self::SignInConfirmationCode => "sign_in_confirmation_code",
            Self::AuthorizationCode => "authorization_code",
            Self::RefreshToken => "refresh_token",
            Self::EmailVerification => "email_verification",
            Self::PasswordReset => "password_reset",
            Self::Email => "email",
            Self::PasswordIdentity => "identity_password",
            Self::GoogleIdentity => "identity_google",
            Self::AppleIdentity => "identity_apple",
            Self::RateLimitSource => "ratelimit_source",
            Self::RateLimitEmail => "ratelimit_email",
            Self::RateLimitClientSource => "ratelimit_client_source",
            Self::RateLimitProviderSource => "ratelimit_provider_source",
        }
    }
}

/// Create a deterministic lookup digest without storing the raw value.
pub fn lookup_digest(secret: &[u8], family: LookupFamily, raw_value: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts keys of any size");
    mac.update(family.label().as_bytes());
    mac.update(b"\0");
    mac.update(raw_value.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
