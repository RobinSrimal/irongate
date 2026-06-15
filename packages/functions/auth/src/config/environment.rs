//! Target runtime auth configuration loader.

use crate::config::account_lifecycle::{AccountLifecycleConfig, AccountLifecycleConfigError};
use crate::config::audit::{AuditConfigError, AuditLogMode};
use crate::config::client_file::{ClientFile, ClientFileError};
use crate::config::signing::{SigningConfig, SigningConfigError};
use crate::config::ttls::{TtlConfig, TtlConfigError};
use crate::core::clients::ClientRegistry;
use crate::crypto::signing::{LocalEs256Signer, SigningMode};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

const DEFAULT_CLIENT_CONFIG_PATH: &str = "auth.clients.toml";
const MIN_LOOKUP_SECRET_BYTES: usize = 32;

/// Secret bytes used for HMAC lookup digests.
#[derive(Clone, PartialEq, Eq)]
pub struct LookupSecret(Vec<u8>);

impl LookupSecret {
    pub fn from_string(value: String) -> Result<Self, RuntimeConfigError> {
        if value.as_bytes().len() < MIN_LOOKUP_SECRET_BYTES {
            return Err(RuntimeConfigError::InvalidLookupSecret);
        }
        Ok(Self(value.into_bytes()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for LookupSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LookupSecret")
            .field("len", &self.0.len())
            .finish_non_exhaustive()
    }
}

/// Runtime foundation loaded before the Lambda starts serving requests.
#[derive(Clone)]
pub struct RuntimeAuthConfig {
    pub client_registry: ClientRegistry,
    pub lookup_secret: LookupSecret,
    pub ttls: TtlConfig,
    pub account_lifecycle: AccountLifecycleConfig,
    pub audit_log_mode: AuditLogMode,
    pub signing: SigningConfig,
    pub signer: LocalEs256Signer,
}

impl fmt::Debug for RuntimeAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeAuthConfig")
            .field("client_registry", &self.client_registry)
            .field("lookup_secret", &self.lookup_secret)
            .field("ttls", &self.ttls)
            .field("account_lifecycle", &self.account_lifecycle)
            .field("audit_log_mode", &self.audit_log_mode)
            .field("signing", &self.signing)
            .field("signer_kid", &self.signer.kid())
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum RuntimeConfigError {
    #[error("client config error: {0}")]
    ClientConfig(#[from] ClientFileError),

    #[error("AUTH_HMAC_LOOKUP_SECRET is required")]
    MissingLookupSecret,

    #[error("AUTH_HMAC_LOOKUP_SECRET must be at least 32 bytes")]
    InvalidLookupSecret,

    #[error("TTL config error: {0}")]
    Ttl(#[from] TtlConfigError),

    #[error("account lifecycle config error: {0}")]
    AccountLifecycle(#[from] AccountLifecycleConfigError),

    #[error("audit config error: {0}")]
    Audit(#[from] AuditConfigError),

    #[error("signing config error: {0}")]
    Signing(#[from] SigningConfigError),

    #[error("secret `{0}` is missing")]
    MissingSecret(String),

    #[error("local ES256 signing key is invalid: {0}")]
    InvalidLocalSigningKey(String),

    #[error("kms-es256 signing is configured but not implemented in this slice")]
    KmsSigningNotImplemented,

    #[error("environment value `{name}` must be a positive integer")]
    InvalidInteger { name: &'static str },
}

impl RuntimeAuthConfig {
    pub fn from_env() -> Result<Self, RuntimeConfigError> {
        let vars: HashMap<String, String> = std::env::vars().collect();
        Self::from_env_map(&vars, |name| std::env::var(name).ok())
    }

    pub fn from_env_map<F>(
        vars: &HashMap<String, String>,
        secret_resolver: F,
    ) -> Result<Self, RuntimeConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let client_config_path = vars
            .get("AUTH_CLIENT_CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CLIENT_CONFIG_PATH));
        let client_config = std::fs::read_to_string(&client_config_path).map_err(|err| {
            ClientFileError::File(format!(
                "failed to read client config `{}`: {err}",
                client_config_path.display()
            ))
        })?;
        let client_file =
            ClientFile::from_toml_str_with_secret_resolver(&client_config, &secret_resolver)?;

        let lookup_secret = vars
            .get("AUTH_HMAC_LOOKUP_SECRET")
            .cloned()
            .ok_or(RuntimeConfigError::MissingLookupSecret)
            .and_then(LookupSecret::from_string)?;

        let ttls = load_ttls(vars)?;
        let account_lifecycle = load_account_lifecycle(vars)?;
        let audit_log_mode = load_audit_mode(vars)?;
        let signing = load_signing(vars)?;
        let signer = load_signer(&signing, &secret_resolver)?;

        Ok(Self {
            client_registry: ClientRegistry::new(client_file.clients),
            lookup_secret,
            ttls,
            account_lifecycle,
            audit_log_mode,
            signing,
            signer,
        })
    }
}

fn load_ttls(vars: &HashMap<String, String>) -> Result<TtlConfig, RuntimeConfigError> {
    let mut ttls = TtlConfig::default();
    ttls.access_token_seconds = optional_u64(vars, "AUTH_ACCESS_TOKEN_TTL_SECONDS")?
        .unwrap_or(ttls.access_token_seconds);
    ttls.id_token_seconds =
        optional_u64(vars, "AUTH_ID_TOKEN_TTL_SECONDS")?.unwrap_or(ttls.id_token_seconds);
    ttls.refresh_token_seconds = optional_u64(vars, "AUTH_REFRESH_TOKEN_TTL_SECONDS")?
        .unwrap_or(ttls.refresh_token_seconds);
    ttls.auth_code_seconds =
        optional_u64(vars, "AUTH_AUTH_CODE_TTL_SECONDS")?.unwrap_or(ttls.auth_code_seconds);
    ttls.authorize_session_seconds =
        optional_u64(vars, "AUTH_AUTHORIZE_SESSION_TTL_SECONDS")?
            .unwrap_or(ttls.authorize_session_seconds);
    ttls.provider_state_seconds = optional_u64(vars, "AUTH_PROVIDER_STATE_TTL_SECONDS")?
        .unwrap_or(ttls.provider_state_seconds);
    ttls.email_verification_seconds =
        optional_u64(vars, "AUTH_EMAIL_VERIFICATION_TTL_SECONDS")?
            .unwrap_or(ttls.email_verification_seconds);
    ttls.password_reset_seconds = optional_u64(vars, "AUTH_PASSWORD_RESET_TTL_SECONDS")?
        .unwrap_or(ttls.password_reset_seconds);
    ttls.validate()?;
    Ok(ttls)
}

fn load_account_lifecycle(
    vars: &HashMap<String, String>,
) -> Result<AccountLifecycleConfig, RuntimeConfigError> {
    let reuse = vars
        .get("AUTH_DELETED_IDENTITY_REUSE")
        .map(String::as_str)
        .unwrap_or("after_retention");
    let retention_days = optional_u32(vars, "AUTH_DELETED_IDENTITY_RETENTION_DAYS")?.unwrap_or(30);
    Ok(AccountLifecycleConfig::from_values(reuse, retention_days)?)
}

fn load_audit_mode(vars: &HashMap<String, String>) -> Result<AuditLogMode, RuntimeConfigError> {
    vars.get("AUTH_AUDIT_LOG_MODE")
        .map(String::as_str)
        .unwrap_or("cloudwatch")
        .parse()
        .map_err(RuntimeConfigError::Audit)
}

fn load_signing(vars: &HashMap<String, String>) -> Result<SigningConfig, RuntimeConfigError> {
    SigningConfig::from_values(
        vars.get("AUTH_SIGNING_MODE")
            .map(String::as_str)
            .unwrap_or("local-es256"),
        vars.get("AUTH_SIGNING_KEY_ID").map(String::as_str),
        vars.get("AUTH_SIGNING_PRIVATE_KEY_SECRET")
            .map(String::as_str),
        vars.get("AUTH_SIGNING_KMS_KEY_ID").map(String::as_str),
    )
    .map_err(RuntimeConfigError::Signing)
}

fn load_signer<F>(
    signing: &SigningConfig,
    secret_resolver: &F,
) -> Result<LocalEs256Signer, RuntimeConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match signing.mode {
        SigningMode::LocalEs256 => {
            let secret_ref = signing
                .local_private_key_secret_ref
                .as_deref()
                .ok_or(SigningConfigError::MissingLocalPrivateKeySecret)?;
            let private_key = secret_resolver(secret_ref)
                .ok_or_else(|| RuntimeConfigError::MissingSecret(secret_ref.to_string()))?;
            LocalEs256Signer::from_private_key_pem(signing.key_id.clone(), private_key)
                .map_err(RuntimeConfigError::InvalidLocalSigningKey)
        }
        SigningMode::KmsEs256 => Err(RuntimeConfigError::KmsSigningNotImplemented),
    }
}

fn optional_u64(
    vars: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<u64>, RuntimeConfigError> {
    vars.get(name)
        .map(|value| parse_positive_u64(value, name))
        .transpose()
}

fn optional_u32(
    vars: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<u32>, RuntimeConfigError> {
    optional_u64(vars, name)?
        .map(|value| u32::try_from(value).map_err(|_| RuntimeConfigError::InvalidInteger { name }))
        .transpose()
}

fn parse_positive_u64(value: &str, name: &'static str) -> Result<u64, RuntimeConfigError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| RuntimeConfigError::InvalidInteger { name })?;
    if parsed == 0 {
        return Err(RuntimeConfigError::InvalidInteger { name });
    }
    Ok(parsed)
}
