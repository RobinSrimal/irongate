//! AWS KMS-backed ES256 token signing.

use crate::core::tokens::{AccessTokenClaims, IdTokenClaims};
use crate::crypto::signing::Jwks;
use crate::crypto::signing::{
    assemble_jwt, jwks_from_public_key_pem, jwt_signing_input, public_key_der_to_pem,
    verify_access_token_with_public_key, TokenSigningError,
};
use async_trait::async_trait;
use aws_sdk_kms::primitives::Blob;
use aws_sdk_kms::types::{MessageType, SigningAlgorithmSpec};
use p256::ecdsa::Signature;
use sha2::{Digest, Sha256};
use std::sync::Arc;

const KMS_ES256_KEY_SPEC: &str = "ECC_NIST_P256";
const KMS_SIGN_VERIFY_USAGE: &str = "SIGN_VERIFY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KmsPublicKey {
    pub der: Vec<u8>,
    pub key_spec: String,
    pub key_usage: String,
}

#[async_trait]
pub trait KmsSigningOperations: Send + Sync {
    async fn get_public_key(&self, key_id: &str) -> Result<KmsPublicKey, String>;
    async fn sign_digest(&self, key_id: &str, digest: &[u8]) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Clone)]
pub struct AwsKmsSigningOperations {
    client: aws_sdk_kms::Client,
}

impl AwsKmsSigningOperations {
    pub fn new(client: aws_sdk_kms::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl KmsSigningOperations for AwsKmsSigningOperations {
    async fn get_public_key(&self, key_id: &str) -> Result<KmsPublicKey, String> {
        let output = self
            .client
            .get_public_key()
            .key_id(key_id)
            .send()
            .await
            .map_err(|err| format!("failed to fetch KMS public key: {err}"))?;

        let der = output
            .public_key()
            .map(|public_key| public_key.as_ref().to_vec())
            .ok_or_else(|| "KMS public key response did not include key material".to_string())?;
        let key_spec = output
            .key_spec()
            .map(|key_spec| key_spec.as_str().to_string())
            .unwrap_or_default();
        let key_usage = output
            .key_usage()
            .map(|key_usage| key_usage.as_str().to_string())
            .unwrap_or_default();

        Ok(KmsPublicKey {
            der,
            key_spec,
            key_usage,
        })
    }

    async fn sign_digest(&self, key_id: &str, digest: &[u8]) -> Result<Vec<u8>, String> {
        let output = self
            .client
            .sign()
            .key_id(key_id)
            .message(Blob::new(digest.to_vec()))
            .message_type(MessageType::Digest)
            .signing_algorithm(SigningAlgorithmSpec::EcdsaSha256)
            .send()
            .await
            .map_err(|err| format!("KMS signing failed: {err}"))?;

        output
            .signature()
            .map(|signature| signature.as_ref().to_vec())
            .ok_or_else(|| "KMS signing response did not include a signature".to_string())
    }
}

#[derive(Clone)]
pub struct KmsEs256Signer {
    kid: String,
    kms_key_id: String,
    public_key_pem: String,
    jwks: Jwks,
    operations: Arc<dyn KmsSigningOperations>,
}

impl std::fmt::Debug for KmsEs256Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KmsEs256Signer")
            .field("kid", &self.kid)
            .field("kms_key_id", &self.kms_key_id)
            .finish_non_exhaustive()
    }
}

impl KmsEs256Signer {
    pub async fn from_operations(
        kid: String,
        kms_key_id: String,
        operations: Arc<dyn KmsSigningOperations>,
    ) -> Result<Self, TokenSigningError> {
        let public_key = operations
            .get_public_key(&kms_key_id)
            .await
            .map_err(TokenSigningError::Kms)?;
        validate_public_key_metadata(&public_key)?;
        let public_key_pem = public_key_der_to_pem(&public_key.der)?;
        let jwks = jwks_from_public_key_pem(&kid, &public_key_pem)?;

        Ok(Self {
            kid,
            kms_key_id,
            public_key_pem,
            jwks,
            operations,
        })
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    pub fn public_key_pem(&self) -> &str {
        &self.public_key_pem
    }

    pub fn jwks(&self) -> Jwks {
        self.jwks.clone()
    }

    pub async fn sign_access_token(
        &self,
        claims: &AccessTokenClaims,
    ) -> Result<String, TokenSigningError> {
        self.sign_claims(claims).await
    }

    pub async fn sign_id_token(&self, claims: &IdTokenClaims) -> Result<String, TokenSigningError> {
        self.sign_claims(claims).await
    }

    pub fn verify_access_token(
        &self,
        token: &str,
        expected_issuer: &str,
        expected_audience: &str,
    ) -> Result<AccessTokenClaims, String> {
        verify_access_token_with_public_key(
            token,
            &self.kid,
            &self.public_key_pem,
            expected_issuer,
            expected_audience,
        )
    }

    async fn sign_claims<T: serde::Serialize>(
        &self,
        claims: &T,
    ) -> Result<String, TokenSigningError> {
        let signing_input = jwt_signing_input(&self.kid, claims)?;
        let digest = Sha256::digest(signing_input.as_bytes());
        let der_signature = self
            .operations
            .sign_digest(&self.kms_key_id, &digest)
            .await
            .map_err(TokenSigningError::Kms)?;
        let jose_signature = der_signature_to_jose(&der_signature)?;
        assemble_jwt(signing_input, &jose_signature)
    }
}

pub fn der_signature_to_jose(der: &[u8]) -> Result<Vec<u8>, TokenSigningError> {
    let signature = Signature::from_der(der)
        .map_err(|err| TokenSigningError::InvalidSignature(err.to_string()))?;
    Ok(signature.to_bytes().to_vec())
}

fn validate_public_key_metadata(public_key: &KmsPublicKey) -> Result<(), TokenSigningError> {
    if public_key.key_spec != KMS_ES256_KEY_SPEC {
        return Err(TokenSigningError::InvalidKmsKey(format!(
            "KMS signing key must use {KMS_ES256_KEY_SPEC}"
        )));
    }

    if public_key.key_usage != KMS_SIGN_VERIFY_USAGE {
        return Err(TokenSigningError::InvalidKmsKey(format!(
            "KMS signing key must use {KMS_SIGN_VERIFY_USAGE}"
        )));
    }

    Ok(())
}
