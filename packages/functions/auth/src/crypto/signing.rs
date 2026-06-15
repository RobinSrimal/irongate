//! Signing facade for first-party tokens and JWKS.
//!
//! This starts with local ES256 keys and leaves a stable seam for KMS-backed
//! signing without exposing private key material through discovery.

use crate::core::tokens::{AccessTokenClaims, IdTokenClaims};
use crate::jwt::{generate_signing_key, to_jwks, Jwks, SigningKey};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
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

    pub fn sign_access_token(
        &self,
        claims: &AccessTokenClaims,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        self.sign_claims(claims)
    }

    pub fn sign_id_token(
        &self,
        claims: &IdTokenClaims,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        self.sign_claims(claims)
    }

    pub fn verify_access_token(
        &self,
        token: &str,
        expected_issuer: &str,
        expected_audience: &str,
    ) -> Result<AccessTokenClaims, String> {
        let header = jsonwebtoken::decode_header(token).map_err(|err| err.to_string())?;
        if header.alg != Algorithm::ES256 {
            return Err("unexpected token algorithm".to_string());
        }
        if header.kid.as_deref() != Some(self.kid()) {
            return Err("unknown signing key".to_string());
        }

        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_issuer(&[expected_issuer]);
        validation.set_audience(&[expected_audience]);
        let key = DecodingKey::from_ec_pem(self.key.public_key_pem.as_bytes())
            .map_err(|err| err.to_string())?;
        let token =
            decode::<AccessTokenClaims>(token, &key, &validation).map_err(|err| err.to_string())?;

        if token.claims.mode != "access" {
            return Err("not an access token".to_string());
        }

        Ok(token.claims)
    }

    fn sign_claims<T: serde::Serialize>(
        &self,
        claims: &T,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key.kid.clone());
        let key = EncodingKey::from_ec_pem(self.key.private_key_pem.as_bytes())?;
        encode(&header, claims, &key)
    }
}
