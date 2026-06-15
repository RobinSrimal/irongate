//! Signing configuration.

use crate::crypto::signing::SigningMode;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningConfig {
    pub mode: SigningMode,
    pub key_id: String,
    pub local_private_key_secret_ref: Option<String>,
    pub kms_key_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum SigningConfigError {
    #[error("unknown signing mode `{0}`")]
    UnknownMode(String),

    #[error("AUTH_SIGNING_KEY_ID is required")]
    MissingKeyId,

    #[error("AUTH_SIGNING_PRIVATE_KEY_SECRET is required for local-es256")]
    MissingLocalPrivateKeySecret,

    #[error("AUTH_SIGNING_KMS_KEY_ID is required for kms-es256")]
    MissingKmsKeyId,
}

impl SigningConfig {
    pub fn from_values(
        mode: &str,
        key_id: Option<&str>,
        local_private_key_secret_ref: Option<&str>,
        kms_key_id: Option<&str>,
    ) -> Result<Self, SigningConfigError> {
        let mode = SigningMode::from_str(mode).map_err(SigningConfigError::UnknownMode)?;
        let key_id = key_id
            .filter(|value| !value.trim().is_empty())
            .ok_or(SigningConfigError::MissingKeyId)?
            .to_string();

        match mode {
            SigningMode::LocalEs256 => {
                let local_private_key_secret_ref = local_private_key_secret_ref
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(SigningConfigError::MissingLocalPrivateKeySecret)?
                    .to_string();
                Ok(Self {
                    mode,
                    key_id,
                    local_private_key_secret_ref: Some(local_private_key_secret_ref),
                    kms_key_id: None,
                })
            }
            SigningMode::KmsEs256 => {
                let kms_key_id = kms_key_id
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(SigningConfigError::MissingKmsKeyId)?
                    .to_string();
                Ok(Self {
                    mode,
                    key_id,
                    local_private_key_secret_ref: None,
                    kms_key_id: Some(kms_key_id),
                })
            }
        }
    }
}
