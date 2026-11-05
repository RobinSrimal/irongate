//! Provider email authority, constructed only after signed ID-token validation.

use super::{
    apple::{apple_identity_digest, AppleIdTokenClaims},
    google::{google_identity_digest, GoogleIdTokenClaims},
};
use crate::{
    core::passwords::normalize_email,
    crypto::hmac_lookup::{lookup_digest, LookupFamily},
    store::IdentityProvider,
};

#[derive(Clone)]
pub struct ProviderVerifiedEmail {
    provider: IdentityProvider,
    identity_digest: String,
    email_digest: String,
    email: String,
}

impl ProviderVerifiedEmail {
    /// The caller must first verify the token's signature, claims and transaction nonce.
    pub fn from_apple_claims(claims: &AppleIdTokenClaims, secret: &[u8]) -> Option<Self> {
        if claims.email_verified != Some(true) {
            return None;
        }
        let email = normalize_email(claims.email.as_deref()?).ok()?;
        Some(Self::new(
            IdentityProvider::Apple,
            apple_identity_digest(secret, &claims.iss, &claims.sub),
            email,
            secret,
        ))
    }

    /// Verified third-party email alone is insufficient: Google must host the address.
    /// The caller must first verify the token's signature, claims and transaction nonce.
    pub fn from_google_claims(claims: &GoogleIdTokenClaims, secret: &[u8]) -> Option<Self> {
        if claims.email_verified != Some(true) {
            return None;
        }
        let email = normalize_email(claims.email.as_deref()?).ok()?;
        if email.rsplit_once('@')?.1 != "gmail.com"
            && !claims.hd.as_deref().is_some_and(valid_hosted_domain)
        {
            return None;
        }
        Some(Self::new(
            IdentityProvider::Google,
            google_identity_digest(secret, &claims.iss, &claims.sub),
            email,
            secret,
        ))
    }

    fn new(
        provider: IdentityProvider,
        identity_digest: String,
        email: String,
        secret: &[u8],
    ) -> Self {
        Self {
            provider,
            identity_digest,
            email_digest: lookup_digest(secret, LookupFamily::Email, &email),
            email,
        }
    }

    pub(crate) fn matches_identity(&self, provider: IdentityProvider, digest: &str) -> bool {
        self.provider == provider && self.identity_digest == digest
    }

    pub(crate) fn email_digest(&self) -> &str {
        &self.email_digest
    }

    pub(crate) fn email(&self) -> &str {
        &self.email
    }
}

fn valid_hosted_domain(domain: &str) -> bool {
    domain.len() <= 253
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn google(email: &str, hd: Option<&str>) -> GoogleIdTokenClaims {
        serde_json::from_value(json!({
            "iss":"https://accounts.google.com", "sub":"signed-subject", "aud":"client",
            "iat":1, "exp":2, "email":email, "email_verified":true, "hd":hd,
        }))
        .unwrap()
    }

    #[test]
    fn hosted_domain_boundaries_and_literal_email_keys() {
        let secret = b"test-only-lookup-secret";
        let max_domain = [
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61),
        ]
        .join(".");
        for hd in ["workspace.example", "A-1.example", max_domain.as_str()] {
            assert!(ProviderVerifiedEmail::from_google_claims(
                &google("alias@example.com", Some(hd)),
                secret
            )
            .is_some());
        }
        for hd in [
            "a".repeat(64),
            format!("{max_domain}x"),
            "a..b".into(),
            "a.b.".into(),
        ] {
            assert!(ProviderVerifiedEmail::from_google_claims(
                &google("alias@example.com", Some(&hd)),
                secret
            )
            .is_none());
        }
        let literal =
            ProviderVerifiedEmail::from_google_claims(&google(" A.B+tag@GMAIL.COM ", None), secret)
                .unwrap();
        assert_eq!(literal.email(), "a.b+tag@gmail.com");
        assert_eq!(
            literal.email_digest(),
            lookup_digest(secret, LookupFamily::Email, "a.b+tag@gmail.com")
        );
        for email in [
            "user@googlemail.com",
            "user@sub.gmail.com",
            "user@gmail.com.evil.test",
        ] {
            assert!(
                ProviderVerifiedEmail::from_google_claims(&google(email, None), secret).is_none()
            );
        }
    }

    #[test]
    fn proofs_are_bound_to_provider_and_identity() {
        let secret = b"test-only-lookup-secret";
        let proof =
            ProviderVerifiedEmail::from_google_claims(&google("user@gmail.com", None), secret)
                .unwrap();
        let digest =
            google_identity_digest(secret, "https://accounts.google.com", "signed-subject");
        assert!(proof.matches_identity(IdentityProvider::Google, &digest));
        assert!(!proof.matches_identity(IdentityProvider::Apple, &digest));
        assert!(!proof.matches_identity(IdentityProvider::Google, "other-identity"));
    }
}
