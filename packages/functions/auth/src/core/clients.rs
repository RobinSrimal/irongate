//! Config-only OAuth client domain types.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Supported OAuth client type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Public,
    Confidential,
}

impl FromStr for ClientType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "public" => Ok(Self::Public),
            "confidential" => Ok(Self::Confidential),
            other => Err(format!("unsupported client_type `{other}`")),
        }
    }
}

/// Grant types supported by the target core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantType {
    AuthorizationCode,
    RefreshToken,
}

impl GrantType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthorizationCode => "authorization_code",
            Self::RefreshToken => "refresh_token",
        }
    }
}

impl FromStr for GrantType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "authorization_code" => Ok(Self::AuthorizationCode),
            "refresh_token" => Ok(Self::RefreshToken),
            "client_credentials" => {
                Err("client_credentials is not supported by the target core".into())
            }
            other => Err(format!("unsupported grant type `{other}`")),
        }
    }
}

/// Supported token endpoint authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenEndpointAuthMethod {
    None,
    ClientSecretBasic,
    ClientSecretPost,
}

impl FromStr for TokenEndpointAuthMethod {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "client_secret_basic" => Ok(Self::ClientSecretBasic),
            "client_secret_post" => Ok(Self::ClientSecretPost),
            other => Err(format!("unsupported token_endpoint_auth_method `{other}`")),
        }
    }
}

/// Runtime client definition loaded from static config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredClient {
    pub client_id: String,
    pub client_type: ClientType,
    pub redirect_uris: Vec<String>,
    pub allowed_grant_types: Vec<GrantType>,
    pub allowed_scopes: Vec<String>,
    pub pkce_required: bool,
    pub token_endpoint_auth_method: TokenEndpointAuthMethod,
    pub client_secret_ref: Option<String>,
    pub client_secret_hash: Option<String>,
}
