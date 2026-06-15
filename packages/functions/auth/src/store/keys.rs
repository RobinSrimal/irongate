//! Typed DynamoDB key helpers for auth records.

/// Logical two-part storage key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreKey {
    pk: String,
    sk: String,
}

impl StoreKey {
    pub fn new(pk: impl Into<String>, sk: impl Into<String>) -> Self {
        Self {
            pk: pk.into(),
            sk: sk.into(),
        }
    }

    pub fn account(subject: &str) -> Self {
        Self::new(format!("account:{subject}"), "meta")
    }

    pub fn identity(provider: &str, identity_digest: &str) -> Self {
        Self::new(format!("identity:{provider}"), identity_digest)
    }

    pub fn authorization_code(code_digest: &str) -> Self {
        Self::new("oauth:code", code_digest)
    }

    pub fn authorize_session(session_digest: &str) -> Self {
        Self::new("oauth:session", session_digest)
    }

    pub fn provider_state(state_digest: &str) -> Self {
        Self::new("provider:state", state_digest)
    }

    pub fn refresh_token(refresh_digest: &str) -> Self {
        Self::new("oauth:refresh", refresh_digest)
    }

    pub fn password_verification(secret_digest: &str) -> Self {
        Self::new("password:verify", secret_digest)
    }

    pub fn password_reset(secret_digest: &str) -> Self {
        Self::new("password:reset", secret_digest)
    }

    pub fn rate_limit(bucket: &str, identifier_digest: &str) -> Self {
        Self::new(format!("ratelimit:{bucket}"), identifier_digest)
    }

    pub fn pk(&self) -> &str {
        &self.pk
    }

    pub fn sk(&self) -> &str {
        &self.sk
    }

    pub fn parts(&self) -> Vec<String> {
        vec![self.pk.clone(), self.sk.clone()]
    }
}
