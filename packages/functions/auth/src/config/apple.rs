//! Sign in with Apple runtime configuration.

use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey;
use std::fmt;
use thiserror::Error;
use url::Url;

pub const APPLE_AUTHORIZATION_URL: &str = "https://appleid.apple.com/auth/authorize";
pub const APPLE_TOKEN_URL: &str = "https://appleid.apple.com/auth/token";
pub const APPLE_ISSUER: &str = "https://appleid.apple.com";
pub const APPLE_JWKS_URI: &str = "https://appleid.apple.com/auth/keys";
pub const APPLE_AUDIENCE: &str = "https://appleid.apple.com";
pub const APPLE_SCOPES: &[&str] = &["name", "email"];
pub const DEFAULT_CLIENT_SECRET_TTL_SECONDS: u64 = 86_400;
pub const MAX_CLIENT_SECRET_TTL_SECONDS: u64 = 15_552_000;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplePrivateKey(String);

impl ApplePrivateKey {
    pub fn new(value: String) -> Result<Self, AppleConfigError> {
        if value.trim().is_empty() {
            return Err(AppleConfigError::MissingRequired);
        }
        SigningKey::from_pkcs8_pem(&value).map_err(|_| AppleConfigError::InvalidPrivateKey)?;
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApplePrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplePrivateKey")
            .field("present", &true)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppleConfig {
    pub client_id: String,
    pub team_id: String,
    pub key_id: String,
    pub private_key: ApplePrivateKey,
    pub client_secret_ttl_seconds: u64,
    pub authorization_url: Url,
    pub token_url: Url,
    pub issuer: String,
    pub jwks_uri: Url,
    pub scopes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum AppleConfigError {
    #[error("Apple OIDC requires AUTH_APPLE_CLIENT_ID, AUTH_APPLE_TEAM_ID, AUTH_APPLE_KEY_ID, and AUTH_APPLE_PRIVATE_KEY_SECRET together")]
    MissingRequired,

    #[error("secret `{0}` is missing")]
    MissingSecret(String),

    #[error("Apple private key must be valid ES256/P-256 PKCS#8 PEM")]
    InvalidPrivateKey,

    #[error("AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS must be between 1 and 15552000")]
    InvalidClientSecretTtl,

    #[error("fixed Apple URL `{name}` is invalid: {source}")]
    InvalidFixedUrl {
        name: &'static str,
        source: url::ParseError,
    },
}

impl AppleConfig {
    pub fn from_values<F>(
        client_id: Option<&str>,
        team_id: Option<&str>,
        key_id: Option<&str>,
        private_key_secret_ref: Option<&str>,
        ttl_seconds: Option<u64>,
        secret_resolver: F,
    ) -> Result<Option<Self>, AppleConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let values = [
            non_empty(client_id),
            non_empty(team_id),
            non_empty(key_id),
            non_empty(private_key_secret_ref),
        ];
        if values.iter().all(Option::is_none) {
            return Ok(None);
        }
        if values.iter().any(Option::is_none) {
            return Err(AppleConfigError::MissingRequired);
        }

        let client_secret_ttl_seconds =
            ttl_seconds.unwrap_or(DEFAULT_CLIENT_SECRET_TTL_SECONDS);
        if client_secret_ttl_seconds == 0
            || client_secret_ttl_seconds > MAX_CLIENT_SECRET_TTL_SECONDS
        {
            return Err(AppleConfigError::InvalidClientSecretTtl);
        }

        let private_key_ref = values[3].expect("checked above");
        let private_key_value = secret_resolver(private_key_ref)
            .ok_or_else(|| AppleConfigError::MissingSecret(private_key_ref.to_string()))?;

        Ok(Some(Self {
            client_id: values[0].expect("checked above").to_string(),
            team_id: values[1].expect("checked above").to_string(),
            key_id: values[2].expect("checked above").to_string(),
            private_key: ApplePrivateKey::new(private_key_value)?,
            client_secret_ttl_seconds,
            authorization_url: fixed_url("authorization_url", APPLE_AUTHORIZATION_URL)?,
            token_url: fixed_url("token_url", APPLE_TOKEN_URL)?,
            issuer: APPLE_ISSUER.to_string(),
            jwks_uri: fixed_url("jwks_uri", APPLE_JWKS_URI)?,
            scopes: APPLE_SCOPES.iter().map(|scope| (*scope).to_string()).collect(),
        }))
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn fixed_url(name: &'static str, value: &str) -> Result<Url, AppleConfigError> {
    Url::parse(value).map_err(|source| AppleConfigError::InvalidFixedUrl { name, source })
}
