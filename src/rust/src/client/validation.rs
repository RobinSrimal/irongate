//! Client validation for OAuth requests.
//!
//! Validates client_id, redirect_uri, client_secret, and grant types
//! on every OAuth request.

use super::registry::get_client;
use super::types::*;
use crate::crypto::secrets::verify_client_secret;
use crate::error::OAuthError;
use crate::storage::StorageAdapter;

/// Validate a client for the /authorize endpoint
pub async fn validate_authorize_request<S: StorageAdapter>(
    storage: &S,
    client_id: &str,
    redirect_uri: &str,
    response_type: &str,
    code_challenge: Option<&str>,
) -> Result<Client, OAuthError> {
    let client = get_client(storage, client_id)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidClient("Client not registered".to_string()))?;

    // Check if client is enabled
    if !client.enabled {
        return Err(OAuthError::InvalidClient("Client is disabled".to_string()));
    }

    // Validate redirect URI (EXACT MATCH - no pattern matching!)
    if !client.redirect_uris.iter().any(|uri| uri == redirect_uri) {
        return Err(OAuthError::InvalidRedirectUri(format!(
            "Redirect URI '{}' not registered for client '{}'",
            redirect_uri, client_id
        )));
    }

    // Validate PKCE requirement
    if client.pkce_required && code_challenge.is_none() {
        return Err(OAuthError::InvalidRequest(
            "PKCE is required for this client (code_challenge missing)".to_string(),
        ));
    }

    // Validate response_type implies grant_type
    let required_grant = match response_type {
        "code" => GrantType::AuthorizationCode,
        "token" => GrantType::AuthorizationCode, // Implicit uses same
        _ => {
            return Err(OAuthError::UnsupportedResponseType(format!(
                "Unsupported response_type: {}",
                response_type
            )))
        }
    };

    if !client.allowed_grant_types.contains(&required_grant) {
        return Err(OAuthError::UnauthorizedClient(
            "Response type not allowed for this client".to_string(),
        ));
    }

    Ok(client)
}

/// Validate a client for the /token endpoint
pub async fn validate_token_request<S: StorageAdapter>(
    storage: &S,
    client_id: &str,
    client_secret: Option<&str>,
    grant_type: &str,
    auth_header: Option<&str>,
) -> Result<Client, OAuthError> {
    let client = get_client(storage, client_id)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidClient("Client not registered".to_string()))?;

    // Check if client is enabled
    if !client.enabled {
        return Err(OAuthError::InvalidClient("Client is disabled".to_string()));
    }

    // Parse grant_type
    let grant = match grant_type {
        "authorization_code" => GrantType::AuthorizationCode,
        "refresh_token" => GrantType::RefreshToken,
        "client_credentials" => GrantType::ClientCredentials,
        _ => {
            return Err(OAuthError::UnsupportedGrantType(format!(
                "Unsupported grant_type: {}",
                grant_type
            )))
        }
    };

    // Validate grant type is allowed
    if !client.allowed_grant_types.contains(&grant) {
        return Err(OAuthError::UnauthorizedClient(
            "Grant type not allowed for this client".to_string(),
        ));
    }

    // Validate client authentication based on type
    match client.client_type {
        ClientType::Confidential => {
            validate_client_authentication(&client, client_secret, auth_header)?;
        }
        ClientType::Public => {
            // Public clients cannot use client_credentials
            if grant == GrantType::ClientCredentials {
                return Err(OAuthError::UnauthorizedClient(
                    "Public clients cannot use client_credentials grant".to_string(),
                ));
            }
        }
    }

    Ok(client)
}

/// Validate client authentication (secret)
fn validate_client_authentication(
    client: &Client,
    client_secret: Option<&str>,
    auth_header: Option<&str>,
) -> Result<(), OAuthError> {
    let secret = match client.token_endpoint_auth_method {
        TokenEndpointAuthMethod::None => {
            return Err(OAuthError::InvalidClient(
                "Confidential client must have auth method".to_string(),
            ));
        }
        TokenEndpointAuthMethod::ClientSecretPost => client_secret.ok_or_else(|| {
            OAuthError::InvalidClient("client_secret required in request body".to_string())
        })?,
        TokenEndpointAuthMethod::ClientSecretBasic => parse_basic_auth(auth_header)?,
    };

    let hash = client
        .client_secret_hash
        .as_ref()
        .ok_or_else(|| OAuthError::ServerError("Client misconfigured".to_string()))?;

    // Use constant-time comparison to prevent timing attacks
    if !verify_client_secret(secret, hash) {
        return Err(OAuthError::InvalidClient(
            "Invalid client secret".to_string(),
        ));
    }

    Ok(())
}

/// Parse Basic auth header and extract password (client_secret)
fn parse_basic_auth(_auth_header: Option<&str>) -> Result<&str, OAuthError> {
    // TODO: Implement proper Basic auth parsing
    // This requires returning an owned String, not a reference
    todo!("Implement Basic auth parsing with owned String return")
}
