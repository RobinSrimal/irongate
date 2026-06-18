//! Config-only OAuth client domain types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

use crate::crypto::password::verify_password;
use crate::error::OAuthError;

/// Supported OAuth client type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Public,
    Confidential,
    Spa,
    NativeMobile,
    NativeDesktop,
    WebConfidential,
}

impl FromStr for ClientType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "public" => Ok(Self::Public),
            "confidential" => Ok(Self::Confidential),
            "spa" => Ok(Self::Spa),
            "native_mobile" => Ok(Self::NativeMobile),
            "native_desktop" => Ok(Self::NativeDesktop),
            "web_confidential" => Ok(Self::WebConfidential),
            other => Err(format!("unsupported client_type `{other}`")),
        }
    }
}

impl ClientType {
    pub fn is_confidential(self) -> bool {
        matches!(self, Self::Confidential | Self::WebConfidential)
    }

    pub fn is_public(self) -> bool {
        !self.is_confidential()
    }

    pub fn is_native_desktop(self) -> bool {
        matches!(self, Self::NativeDesktop)
    }

    pub fn requires_allowed_origins(self) -> bool {
        matches!(self, Self::Spa)
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
    pub allowed_origins: Vec<String>,
    pub allowed_grant_types: Vec<GrantType>,
    pub allowed_scopes: Vec<String>,
    pub pkce_required: bool,
    pub token_endpoint_auth_method: TokenEndpointAuthMethod,
    pub client_secret_ref: Option<String>,
    pub client_secret_hash: Option<String>,
}

/// Read-only registry for config-defined OAuth clients.
#[derive(Debug, Clone)]
pub struct ClientRegistry {
    clients: HashMap<String, ConfiguredClient>,
}

#[derive(Debug, Error)]
pub enum ClientRegistryError {
    #[error("client `{0}` is not registered")]
    UnknownClient(String),

    #[error("redirect URI `{redirect_uri}` is not registered for client `{client_id}`")]
    InvalidRedirectUri {
        client_id: String,
        redirect_uri: String,
    },

    #[error("response type `{0}` is not supported")]
    UnsupportedResponseType(String),

    #[error("grant type `{0}` is not supported")]
    UnsupportedGrantType(String),

    #[error("client `{client_id}` is not allowed to use grant `{grant}`")]
    UnauthorizedGrant { client_id: String, grant: String },

    #[error("PKCE is required for client `{0}`")]
    PkceRequired(String),

    #[error("client `{0}` requires a client secret")]
    ClientSecretRequired(String),

    #[error("client `{0}` secret is invalid")]
    InvalidClientSecret(String),

    #[error("client `{0}` is missing its derived secret hash")]
    MissingClientSecretHash(String),
}

impl ClientRegistry {
    pub fn new(clients: Vec<ConfiguredClient>) -> Self {
        Self {
            clients: clients
                .into_iter()
                .map(|client| (client.client_id.clone(), client))
                .collect(),
        }
    }

    pub fn get(&self, client_id: &str) -> Option<&ConfiguredClient> {
        self.clients.get(client_id)
    }

    pub fn browser_allowed_origins(&self) -> Vec<String> {
        self.clients
            .values()
            .flat_map(|client| client.allowed_origins.iter().cloned())
            .collect()
    }

    pub fn validate_authorize_request(
        &self,
        client_id: &str,
        redirect_uri: &str,
        response_type: &str,
        code_challenge: Option<&str>,
    ) -> Result<&ConfiguredClient, ClientRegistryError> {
        let client = self
            .get(client_id)
            .ok_or_else(|| ClientRegistryError::UnknownClient(client_id.to_string()))?;

        if !redirect_uri_allowed(client, redirect_uri) {
            return Err(ClientRegistryError::InvalidRedirectUri {
                client_id: client_id.to_string(),
                redirect_uri: redirect_uri.to_string(),
            });
        }

        let grant = match response_type {
            "code" => GrantType::AuthorizationCode,
            other => {
                return Err(ClientRegistryError::UnsupportedResponseType(
                    other.to_string(),
                ))
            }
        };

        self.ensure_grant_allowed(client, grant)?;

        if client.pkce_required && code_challenge.is_none() {
            return Err(ClientRegistryError::PkceRequired(client_id.to_string()));
        }

        Ok(client)
    }

    pub fn validate_token_grant(
        &self,
        client_id: &str,
        grant_type: &str,
    ) -> Result<&ConfiguredClient, ClientRegistryError> {
        let grant = GrantType::from_str(grant_type)
            .map_err(|_| ClientRegistryError::UnsupportedGrantType(grant_type.to_string()))?;
        self.validate_token_request(client_id, grant, None)
    }

    pub fn validate_token_request(
        &self,
        client_id: &str,
        grant: GrantType,
        provided_secret: Option<&str>,
    ) -> Result<&ConfiguredClient, ClientRegistryError> {
        let client = self
            .get(client_id)
            .ok_or_else(|| ClientRegistryError::UnknownClient(client_id.to_string()))?;

        self.ensure_grant_allowed(client, grant)?;

        if client.client_type.is_confidential() {
            let provided_secret = provided_secret
                .ok_or_else(|| ClientRegistryError::ClientSecretRequired(client_id.to_string()))?;
            let hash = client.client_secret_hash.as_deref().ok_or_else(|| {
                ClientRegistryError::MissingClientSecretHash(client_id.to_string())
            })?;
            if !verify_password(provided_secret, hash) {
                return Err(ClientRegistryError::InvalidClientSecret(
                    client_id.to_string(),
                ));
            }
        }

        Ok(client)
    }

    fn ensure_grant_allowed(
        &self,
        client: &ConfiguredClient,
        grant: GrantType,
    ) -> Result<(), ClientRegistryError> {
        if !client.allowed_grant_types.contains(&grant) {
            return Err(ClientRegistryError::UnauthorizedGrant {
                client_id: client.client_id.clone(),
                grant: grant.as_str().to_string(),
            });
        }

        Ok(())
    }
}

fn redirect_uri_allowed(client: &ConfiguredClient, redirect_uri: &str) -> bool {
    if client.redirect_uris.iter().any(|uri| uri == redirect_uri) {
        return true;
    }

    client.client_type.is_native_desktop()
        && client
            .redirect_uris
            .iter()
            .any(|registered| native_desktop_loopback_redirect_matches(registered, redirect_uri))
}

fn native_desktop_loopback_redirect_matches(registered: &str, requested: &str) -> bool {
    let Ok(registered) = url::Url::parse(registered) else {
        return false;
    };
    let Ok(requested) = url::Url::parse(requested) else {
        return false;
    };

    registered.scheme() == "http"
        && requested.scheme() == "http"
        && registered.port().is_none()
        && requested.port().is_some()
        && registered.host_str() == requested.host_str()
        && is_loopback_host(registered.host_str())
        && is_loopback_host(requested.host_str())
        && registered.path() == requested.path()
        && registered.query() == requested.query()
        && requested.fragment().is_none()
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost") | Some("127.0.0.1") | Some("::1"))
}

impl From<ClientRegistryError> for OAuthError {
    fn from(err: ClientRegistryError) -> Self {
        match err {
            ClientRegistryError::UnknownClient(_)
            | ClientRegistryError::ClientSecretRequired(_)
            | ClientRegistryError::InvalidClientSecret(_)
            | ClientRegistryError::MissingClientSecretHash(_) => {
                OAuthError::InvalidClient(err.to_string())
            }
            ClientRegistryError::InvalidRedirectUri { .. } => {
                OAuthError::InvalidRedirectUri(err.to_string())
            }
            ClientRegistryError::UnsupportedResponseType(_) => {
                OAuthError::UnsupportedResponseType(err.to_string())
            }
            ClientRegistryError::UnsupportedGrantType(_) => {
                OAuthError::UnsupportedGrantType(err.to_string())
            }
            ClientRegistryError::UnauthorizedGrant { .. } => {
                OAuthError::UnauthorizedClient(err.to_string())
            }
            ClientRegistryError::PkceRequired(_) => OAuthError::InvalidRequest(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(client_type: ClientType, redirect_uris: Vec<&str>) -> ConfiguredClient {
        ConfiguredClient {
            client_id: "client".to_string(),
            client_type,
            redirect_uris: redirect_uris.into_iter().map(str::to_string).collect(),
            allowed_origins: Vec::new(),
            allowed_grant_types: vec![GrantType::AuthorizationCode],
            allowed_scopes: vec!["openid".to_string()],
            pkce_required: true,
            token_endpoint_auth_method: TokenEndpointAuthMethod::None,
            client_secret_ref: None,
            client_secret_hash: None,
        }
    }

    #[test]
    fn native_desktop_allows_dynamic_loopback_port() {
        let registry = ClientRegistry::new(vec![client(
            ClientType::NativeDesktop,
            vec!["http://127.0.0.1/oauth/callback"],
        )]);

        assert!(registry
            .validate_authorize_request(
                "client",
                "http://127.0.0.1:49152/oauth/callback",
                "code",
                Some("challenge"),
            )
            .is_ok());
        assert!(registry
            .validate_authorize_request(
                "client",
                "http://127.0.0.1:49152/other",
                "code",
                Some("challenge"),
            )
            .is_err());
        assert!(registry
            .validate_authorize_request(
                "client",
                "http://192.168.0.1:49152/oauth/callback",
                "code",
                Some("challenge"),
            )
            .is_err());
    }

    #[test]
    fn non_desktop_clients_keep_exact_redirect_matching() {
        let registry = ClientRegistry::new(vec![client(
            ClientType::Spa,
            vec!["http://127.0.0.1/oauth/callback"],
        )]);

        assert!(registry
            .validate_authorize_request(
                "client",
                "http://127.0.0.1:49152/oauth/callback",
                "code",
                Some("challenge"),
            )
            .is_err());
    }
}
