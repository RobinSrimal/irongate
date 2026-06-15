//! Signing facade for first-party tokens and JWKS.
//!
//! This starts with local ES256 keys and leaves a stable seam for KMS-backed
//! signing without exposing private key material through discovery.

use crate::jwt::{generate_signing_key, to_jwks, Jwks, SigningKey};
use p256::ecdsa::SigningKey as P256SigningKey;
use p256::pkcs8::{DecodePrivateKey, EncodePublicKey};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningMode {
    LocalEs256,
    KmsEs256,
}

impl FromStr for SigningMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local-es256" => Ok(Self::LocalEs256),
            "kms-es256" => Ok(Self::KmsEs256),
            other => Err(other.to_string()),
        }
    }
}

/// Local ES256 signer loaded from key material or generated for tests.
#[derive(Debug, Clone)]
pub struct LocalEs256Signer {
    key: SigningKey,
}

impl LocalEs256Signer {
    pub fn generate() -> Result<Self, String> {
        Ok(Self {
            key: generate_signing_key()?,
        })
    }

    pub fn from_private_key_pem(
        kid: impl Into<String>,
        private_key_pem: impl Into<String>,
    ) -> Result<Self, String> {
        let private_key_pem = private_key_pem.into();
        let signing_key = P256SigningKey::from_pkcs8_pem(&private_key_pem)
            .map_err(|err| format!("invalid ES256 private key: {err}"))?;
        let public_key_pem = signing_key
            .verifying_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .map_err(|err| format!("failed to derive ES256 public key: {err}"))?;
        let now = chrono::Utc::now();

        Ok(Self {
            key: SigningKey {
                kid: kid.into(),
                private_key_pem,
                public_key_pem,
                created_at: now,
                expires_at: now + chrono::Duration::days(90),
            },
        })
    }

    pub fn kid(&self) -> &str {
        &self.key.kid
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.key
    }

    pub fn jwks(&self) -> Jwks {
        to_jwks(std::slice::from_ref(&self.key))
    }
}
