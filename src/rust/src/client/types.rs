//! Client type definitions.
//!
//! Defines the structure of registered OAuth clients.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Registered OAuth client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    /// Unique client identifier
    pub client_id: String,

    /// Client type (public or confidential)
    pub client_type: ClientType,

    /// Hashed client secret (Argon2, for confidential clients only)
    pub client_secret_hash: Option<String>,

    /// Allowed redirect URIs (exact match required)
    pub redirect_uris: Vec<String>,

    /// Allowed OAuth grant types
    pub allowed_grant_types: Vec<GrantType>,

    /// Allowed OAuth scopes
    pub allowed_scopes: Vec<String>,

    /// Whether PKCE is required (default: true)
    pub pkce_required: bool,

    /// Token endpoint authentication method
    pub token_endpoint_auth_method: TokenEndpointAuthMethod,

    /// Custom access token TTL (overrides default)
    pub access_token_ttl: Option<u64>,

    /// Custom refresh token TTL (overrides default)
    pub refresh_token_ttl: Option<u64>,

    /// When the client was created
    pub created_at: DateTime<Utc>,

    /// When the client was last updated
    pub updated_at: DateTime<Utc>,

    /// Whether the client is enabled
    pub enabled: bool,
}

/// Client type classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    /// Public clients (SPAs, mobile apps, CLIs) - cannot keep secrets
    Public,
    /// Confidential clients (backend apps) - can keep secrets
    Confidential,
}

/// OAuth 2.0 grant types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrantType {
    /// Authorization code grant
    AuthorizationCode,
    /// Refresh token grant
    RefreshToken,
    /// Client credentials grant (confidential clients only)
    ClientCredentials,
}

/// Token endpoint authentication methods
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenEndpointAuthMethod {
    /// No authentication (public clients)
    None,
    /// Client secret in request body
    ClientSecretPost,
    /// Client secret in Authorization header (Basic auth)
    ClientSecretBasic,
}

/// Request to create a new client
#[derive(Debug, Deserialize)]
pub struct CreateClientRequest {
    pub client_id: String,

    pub client_type: ClientType,

    pub redirect_uris: Vec<String>,

    pub allowed_grant_types: Vec<GrantType>,

    pub allowed_scopes: Option<Vec<String>>,

    /// Whether PKCE is required (default: true)
    pub pkce_required: Option<bool>,

    pub access_token_ttl: Option<u64>,
    pub refresh_token_ttl: Option<u64>,
}

/// Response after creating a client
#[derive(Debug, Serialize)]
pub struct CreateClientResponse {
    pub client_id: String,
    /// Client secret (only returned once for confidential clients)
    pub client_secret: Option<String>,
    pub client_type: ClientType,
    pub created_at: DateTime<Utc>,
}

/// Request to update a client
#[derive(Debug, Deserialize)]
pub struct UpdateClientRequest {
    pub redirect_uris: Option<Vec<String>>,
    pub allowed_grant_types: Option<Vec<GrantType>>,
    pub allowed_scopes: Option<Vec<String>>,
    pub pkce_required: Option<bool>,
    pub access_token_ttl: Option<u64>,
    pub refresh_token_ttl: Option<u64>,
    pub enabled: Option<bool>,
}
