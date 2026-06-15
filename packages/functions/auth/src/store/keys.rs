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

    pub fn identity_by_subject(subject: &str, provider: &str, identity_digest: &str) -> Self {
        Self::new(
            Self::identity_by_subject_pk(subject),
            format!("{provider}:{identity_digest}"),
        )
    }

    pub fn identity_by_subject_pk(subject: &str) -> String {
        format!("identity_by_subject:{subject}")
    }

    pub fn password_user(email_digest: &str) -> Self {
        Self::new("password:user", email_digest)
    }

    pub fn password_user_by_subject(subject: &str, email_digest: &str) -> Self {
        Self::new(Self::password_user_by_subject_pk(subject), email_digest)
    }

    pub fn password_user_by_subject_pk(subject: &str) -> String {
        format!("password_user_by_subject:{subject}")
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

    pub fn refresh_family(family_id: &str) -> Self {
        Self::new("oauth:refresh_family", family_id)
    }

    pub fn refresh_by_subject(subject: &str, refresh_digest: &str) -> Self {
        Self::new(Self::refresh_by_subject_pk(subject), refresh_digest)
    }

    pub fn refresh_by_subject_pk(subject: &str) -> String {
        format!("oauth:refresh_by_subject:{subject}")
    }

    pub fn refresh_by_client(client_id: &str, refresh_digest: &str) -> Self {
        Self::new(
            format!("oauth:refresh_by_client:{client_id}"),
            refresh_digest,
        )
    }

    pub fn password_verification(secret_digest: &str) -> Self {
        Self::new("password:verify", secret_digest)
    }

    pub fn password_reset(secret_digest: &str) -> Self {
        Self::new("password:reset", secret_digest)
    }

    pub fn password_reset_by_subject(subject: &str, secret_digest: &str) -> Self {
        Self::new(Self::password_reset_by_subject_pk(subject), secret_digest)
    }

    pub fn password_reset_by_subject_pk(subject: &str) -> String {
        format!("password_reset_by_subject:{subject}")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};

    #[test]
    fn hmac_key_helpers_never_store_raw_bearer_values() {
        let secret = b"template-local-secret-with-enough-bytes";
        let raw_authorization_code = "ig_code_raw_secret_value";
        let raw_refresh_token = "ig_refresh_raw_secret_value";

        let code_digest = lookup_digest(
            secret,
            LookupFamily::AuthorizationCode,
            raw_authorization_code,
        );
        let refresh_digest = lookup_digest(secret, LookupFamily::RefreshToken, raw_refresh_token);

        assert_ne!(code_digest, refresh_digest);
        assert_eq!(
            code_digest,
            lookup_digest(
                secret,
                LookupFamily::AuthorizationCode,
                raw_authorization_code
            )
        );

        let code_key = StoreKey::authorization_code(&code_digest);
        let refresh_key = StoreKey::refresh_token(&refresh_digest);

        for key in [code_key, refresh_key] {
            assert!(!key.pk().contains(raw_authorization_code));
            assert!(!key.sk().contains(raw_authorization_code));
            assert!(!key.pk().contains(raw_refresh_token));
            assert!(!key.sk().contains(raw_refresh_token));
        }
    }
}
