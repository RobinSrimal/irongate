//! Google OIDC runtime configuration.

use std::fmt;
use thiserror::Error;
use url::Url;

pub const GOOGLE_AUTHORIZATION_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_ISSUER: &str = "https://accounts.google.com";
pub const GOOGLE_JWKS_URI: &str = "https://www.googleapis.com/oauth2/v3/certs";
pub const GOOGLE_SCOPES: &[&str] = &["openid", "email", "profile"];

#[derive(Clone, PartialEq, Eq)]
pub struct GoogleClientSecret(String);

impl GoogleClientSecret {
    pub fn new(value: String) -> Result<Self, GoogleConfigError> {
        if value.trim().is_empty() {
            return Err(GoogleConfigError::MissingPair);
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GoogleClientSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleClientSecret")
            .field("present", &true)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: GoogleClientSecret,
    pub authorization_url: Url,
    pub token_url: Url,
    pub issuer: String,
    pub jwks_uri: Url,
    pub scopes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum GoogleConfigError {
    #[error("Google OIDC requires both AUTH_GOOGLE_CLIENT_ID and AUTH_GOOGLE_CLIENT_SECRET")]
    MissingPair,

    #[error("fixed Google URL `{name}` is invalid: {source}")]
    InvalidFixedUrl {
        name: &'static str,
        source: url::ParseError,
    },
}

impl GoogleConfig {
    pub fn from_values(
        client_id: Option<&str>,
        client_secret: Option<&str>,
    ) -> Result<Option<Self>, GoogleConfigError> {
        match (non_empty(client_id), non_empty(client_secret)) {
            (None, None) => Ok(None),
            (Some(client_id), Some(client_secret)) => Ok(Some(Self {
                client_id: client_id.to_string(),
                client_secret: GoogleClientSecret::new(client_secret.to_string())?,
                authorization_url: fixed_url("authorization_url", GOOGLE_AUTHORIZATION_URL)?,
                token_url: fixed_url("token_url", GOOGLE_TOKEN_URL)?,
                issuer: GOOGLE_ISSUER.to_string(),
                jwks_uri: fixed_url("jwks_uri", GOOGLE_JWKS_URI)?,
                scopes: GOOGLE_SCOPES.iter().map(|scope| (*scope).to_string()).collect(),
            })),
            _ => Err(GoogleConfigError::MissingPair),
        }
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn fixed_url(name: &'static str, value: &str) -> Result<Url, GoogleConfigError> {
    Url::parse(value).map_err(|source| GoogleConfigError::InvalidFixedUrl { name, source })
}
