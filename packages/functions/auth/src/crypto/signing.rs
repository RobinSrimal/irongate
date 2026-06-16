//! Signing facade for first-party tokens and JWKS.
//!
//! Local ES256 and KMS ES256 share the same JWT serialization path so the
//! configured signing mode changes key custody, not token semantics.

use crate::core::tokens::{AccessTokenClaims, IdTokenClaims};
#[allow(unused_imports)]
pub use crate::crypto::kms_signing::{
    der_signature_to_jose, AwsKmsSigningOperations, KmsEs256Signer, KmsPublicKey,
    KmsSigningOperations,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey as P256SigningKey, VerifyingKey};
use p256::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use thiserror::Error;

/// Local ES256 signing key material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKey {
    pub kid: String,
    pub private_key_pem: String,
    pub public_key_pem: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// JWKS response.
#[derive(Debug, Clone, Serialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// Individual ES256 JWK.
#[derive(Debug, Clone, Serialize)]
pub struct Jwk {
    pub kty: String,
    pub alg: String,
    pub use_: String,
    pub kid: String,
    pub crv: String,
    pub x: String,
    pub y: String,
}

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

#[derive(Debug, Error)]
pub enum TokenSigningError {
    #[error("JWT serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid ES256 private key: {0}")]
    InvalidPrivateKey(String),

    #[error("invalid ES256 public key: {0}")]
    InvalidPublicKey(String),

    #[error("invalid ES256 signature: {0}")]
    InvalidSignature(String),

    #[error("KMS signing error: {0}")]
    Kms(String),

    #[error("invalid KMS signing key: {0}")]
    InvalidKmsKey(String),
}

#[derive(Debug, Clone)]
pub enum TokenSigner {
    Local(LocalEs256Signer),
    Kms(KmsEs256Signer),
}

impl TokenSigner {
    pub fn kid(&self) -> &str {
        match self {
            Self::Local(signer) => signer.kid(),
            Self::Kms(signer) => signer.kid(),
        }
    }

    pub fn public_key_pem(&self) -> &str {
        match self {
            Self::Local(signer) => signer.public_key_pem(),
            Self::Kms(signer) => signer.public_key_pem(),
        }
    }

    pub fn jwks(&self) -> Jwks {
        match self {
            Self::Local(signer) => signer.jwks(),
            Self::Kms(signer) => signer.jwks(),
        }
    }

    pub async fn sign_access_token(
        &self,
        claims: &AccessTokenClaims,
    ) -> Result<String, TokenSigningError> {
        match self {
            Self::Local(signer) => signer.sign_access_token(claims),
            Self::Kms(signer) => signer.sign_access_token(claims).await,
        }
    }

    pub async fn sign_id_token(&self, claims: &IdTokenClaims) -> Result<String, TokenSigningError> {
        match self {
            Self::Local(signer) => signer.sign_id_token(claims),
            Self::Kms(signer) => signer.sign_id_token(claims).await,
        }
    }

    pub fn verify_access_token(
        &self,
        token: &str,
        expected_issuer: &str,
        expected_audience: &str,
    ) -> Result<AccessTokenClaims, String> {
        match self {
            Self::Local(signer) => {
                signer.verify_access_token(token, expected_issuer, expected_audience)
            }
            Self::Kms(signer) => {
                signer.verify_access_token(token, expected_issuer, expected_audience)
            }
        }
    }
}

impl From<LocalEs256Signer> for TokenSigner {
    fn from(value: LocalEs256Signer) -> Self {
        Self::Local(value)
    }
}

impl From<KmsEs256Signer> for TokenSigner {
    fn from(value: KmsEs256Signer) -> Self {
        Self::Kms(value)
    }
}

/// Local ES256 signer loaded from key material or generated for tests.
#[derive(Debug, Clone)]
pub struct LocalEs256Signer {
    key: SigningKey,
    jwks: Jwks,
}

impl LocalEs256Signer {
    pub fn generate() -> Result<Self, String> {
        let signing_key = P256SigningKey::random(&mut rand::rngs::OsRng);
        let private_key_pem = signing_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|err| format!("failed to encode ES256 private key: {err}"))?
            .to_string();
        let public_key_pem = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|err| format!("failed to encode ES256 public key: {err}"))?;
        let now = Utc::now();

        Self::from_signing_key(SigningKey {
            kid: uuid::Uuid::new_v4().to_string(),
            private_key_pem,
            public_key_pem,
            created_at: now,
            expires_at: now + Duration::days(90),
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
            .to_public_key_pem(LineEnding::LF)
            .map_err(|err| format!("failed to derive ES256 public key: {err}"))?;
        let now = Utc::now();

        Self::from_signing_key(SigningKey {
            kid: kid.into(),
            private_key_pem,
            public_key_pem,
            created_at: now,
            expires_at: now + Duration::days(90),
        })
    }

    pub fn kid(&self) -> &str {
        &self.key.kid
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.key
    }

    pub fn public_key_pem(&self) -> &str {
        &self.key.public_key_pem
    }

    pub fn jwks(&self) -> Jwks {
        self.jwks.clone()
    }

    pub fn sign_access_token(
        &self,
        claims: &AccessTokenClaims,
    ) -> Result<String, TokenSigningError> {
        self.sign_claims(claims)
    }

    pub fn sign_id_token(&self, claims: &IdTokenClaims) -> Result<String, TokenSigningError> {
        self.sign_claims(claims)
    }

    pub fn verify_access_token(
        &self,
        token: &str,
        expected_issuer: &str,
        expected_audience: &str,
    ) -> Result<AccessTokenClaims, String> {
        verify_access_token_with_public_key(
            token,
            self.kid(),
            self.public_key_pem(),
            expected_issuer,
            expected_audience,
        )
    }

    fn from_signing_key(key: SigningKey) -> Result<Self, String> {
        let jwks = jwks_from_public_key_pem(&key.kid, &key.public_key_pem)
            .map_err(|err| err.to_string())?;
        Ok(Self { key, jwks })
    }

    fn sign_claims<T: Serialize>(&self, claims: &T) -> Result<String, TokenSigningError> {
        let signing_input = jwt_signing_input(self.kid(), claims)?;
        let signing_key = P256SigningKey::from_pkcs8_pem(&self.key.private_key_pem)
            .map_err(|err| TokenSigningError::InvalidPrivateKey(err.to_string()))?;
        let signature: Signature = signing_key.sign(signing_input.as_bytes());
        assemble_jwt(signing_input, &signature.to_bytes())
    }
}

#[derive(Serialize)]
struct Es256JwtHeader<'a> {
    alg: &'static str,
    typ: &'static str,
    kid: &'a str,
}

pub(crate) fn jwt_signing_input<T: Serialize>(
    kid: &str,
    claims: &T,
) -> Result<String, TokenSigningError> {
    let header = Es256JwtHeader {
        alg: "ES256",
        typ: "JWT",
        kid,
    };
    let header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header)?);
    let claims = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims)?);
    Ok(format!("{header}.{claims}"))
}

pub(crate) fn assemble_jwt(
    signing_input: String,
    raw_signature: &[u8],
) -> Result<String, TokenSigningError> {
    if raw_signature.len() != 64 {
        return Err(TokenSigningError::InvalidSignature(
            "ES256 signatures must be 64 bytes".to_string(),
        ));
    }
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(raw_signature)
    ))
}

pub(crate) fn public_key_der_to_pem(der: &[u8]) -> Result<String, TokenSigningError> {
    let public_key = p256::PublicKey::from_public_key_der(der)
        .map_err(|err| TokenSigningError::InvalidPublicKey(err.to_string()))?;
    public_key
        .to_public_key_pem(LineEnding::LF)
        .map_err(|err| TokenSigningError::InvalidPublicKey(err.to_string()))
}

pub(crate) fn jwks_from_public_key_pem(
    kid: &str,
    public_key_pem: &str,
) -> Result<Jwks, TokenSigningError> {
    let verifying_key = VerifyingKey::from_public_key_pem(public_key_pem)
        .map_err(|err| TokenSigningError::InvalidPublicKey(err.to_string()))?;
    let point = verifying_key.to_encoded_point(false);
    let x = point
        .x()
        .ok_or_else(|| TokenSigningError::InvalidPublicKey("missing P-256 x coordinate".into()))?;
    let y = point
        .y()
        .ok_or_else(|| TokenSigningError::InvalidPublicKey("missing P-256 y coordinate".into()))?;

    Ok(Jwks {
        keys: vec![Jwk {
            kty: "EC".to_string(),
            alg: "ES256".to_string(),
            use_: "sig".to_string(),
            kid: kid.to_string(),
            crv: "P-256".to_string(),
            x: URL_SAFE_NO_PAD.encode(x),
            y: URL_SAFE_NO_PAD.encode(y),
        }],
    })
}

pub(crate) fn verify_access_token_with_public_key(
    token: &str,
    kid: &str,
    public_key_pem: &str,
    expected_issuer: &str,
    expected_audience: &str,
) -> Result<AccessTokenClaims, String> {
    let header = jsonwebtoken::decode_header(token).map_err(|err| err.to_string())?;
    if header.alg != Algorithm::ES256 {
        return Err("unexpected token algorithm".to_string());
    }
    if header.kid.as_deref() != Some(kid) {
        return Err("unknown signing key".to_string());
    }

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_issuer(&[expected_issuer]);
    validation.set_audience(&[expected_audience]);
    let key = DecodingKey::from_ec_pem(public_key_pem.as_bytes()).map_err(|err| err.to_string())?;
    let token = decode::<Value>(token, &key, &validation).map_err(|err| err.to_string())?;

    if token
        .claims
        .get("mode")
        .and_then(|mode| mode.as_str())
        != Some("access")
    {
        return Err("not an access token".to_string());
    }

    serde_json::from_value(token.claims).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_es256_signer_jwks_contains_public_material_only() {
        let signer = LocalEs256Signer::generate().expect("signer");
        let jwks_json = serde_json::to_value(signer.jwks()).expect("jwks json");
        let first_key = jwks_json["keys"][0].as_object().expect("jwk object");

        assert_eq!(
            first_key.get("kid").and_then(|v| v.as_str()),
            Some(signer.kid())
        );
        assert_eq!(first_key.get("alg").and_then(|v| v.as_str()), Some("ES256"));
        assert!(first_key.contains_key("x"));
        assert!(first_key.contains_key("y"));
        assert!(!jwks_json.to_string().contains("PRIVATE KEY"));
        assert!(!first_key.contains_key("d"));
    }
}
