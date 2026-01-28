//! Client registry operations.
//!
//! CRUD operations for OAuth client management.

use chrono::Utc;
use serde_json::json;

use super::types::*;
use crate::crypto::secrets::{generate_client_secret, hash_client_secret};
use crate::error::{OAuthError, StorageError};
use crate::storage::StorageAdapter;

/// Get a client by ID
pub async fn get_client<S: StorageAdapter>(
    storage: &S,
    client_id: &str,
) -> Result<Option<Client>, StorageError> {
    let key = ["client", client_id, "config"];
    match storage.get(&key).await? {
        Some(value) => {
            let client: Client =
                serde_json::from_value(value).map_err(|e| StorageError::DynamoDB(e.to_string()))?;
            Ok(Some(client))
        }
        None => Ok(None),
    }
}

/// Create a new client
pub async fn create_client<S: StorageAdapter>(
    storage: &S,
    request: CreateClientRequest,
) -> Result<CreateClientResponse, OAuthError> {
    // Check if client already exists
    let key = ["client", &request.client_id, "config"];
    if storage.get(&key).await.map_err(|e| OAuthError::ServerError(e.to_string()))?.is_some() {
        return Err(OAuthError::InvalidClient(format!(
            "Client '{}' already exists",
            request.client_id
        )));
    }

    // Generate client secret for confidential clients
    let (secret, secret_hash) = if request.client_type == ClientType::Confidential {
        let secret = generate_client_secret();
        let hash = hash_client_secret(&secret)
            .map_err(|e| OAuthError::ServerError(e.to_string()))?;
        (Some(secret), Some(hash))
    } else {
        (None, None)
    };

    let now = Utc::now();
    let client = Client {
        client_id: request.client_id.clone(),
        client_type: request.client_type,
        client_secret_hash: secret_hash,
        redirect_uris: request.redirect_uris,
        allowed_grant_types: request.allowed_grant_types,
        allowed_scopes: request.allowed_scopes.unwrap_or_default(),
        pkce_required: request.pkce_required.unwrap_or(true), // Default: PKCE required
        token_endpoint_auth_method: if request.client_type == ClientType::Confidential {
            TokenEndpointAuthMethod::ClientSecretPost
        } else {
            TokenEndpointAuthMethod::None
        },
        access_token_ttl: request.access_token_ttl,
        refresh_token_ttl: request.refresh_token_ttl,
        created_at: now,
        updated_at: now,
        enabled: true,
    };

    // Store client
    let value = serde_json::to_value(&client)
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;
    storage
        .set(&key, value, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(CreateClientResponse {
        client_id: client.client_id,
        client_secret: secret, // Only returned once!
        client_type: client.client_type,
        created_at: client.created_at,
    })
}

/// Update an existing client
pub async fn update_client<S: StorageAdapter>(
    storage: &S,
    client_id: &str,
    request: UpdateClientRequest,
) -> Result<Client, OAuthError> {
    let key = ["client", client_id, "config"];

    let mut client = get_client(storage, client_id)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidClient(format!("Client '{}' not found", client_id)))?;

    // Apply updates
    if let Some(uris) = request.redirect_uris {
        client.redirect_uris = uris;
    }
    if let Some(grants) = request.allowed_grant_types {
        client.allowed_grant_types = grants;
    }
    if let Some(scopes) = request.allowed_scopes {
        client.allowed_scopes = scopes;
    }
    if let Some(pkce) = request.pkce_required {
        client.pkce_required = pkce;
    }
    if let Some(ttl) = request.access_token_ttl {
        client.access_token_ttl = Some(ttl);
    }
    if let Some(ttl) = request.refresh_token_ttl {
        client.refresh_token_ttl = Some(ttl);
    }
    if let Some(enabled) = request.enabled {
        client.enabled = enabled;
    }

    client.updated_at = Utc::now();

    // Save updated client
    let value = serde_json::to_value(&client)
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;
    storage
        .set(&key, value, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(client)
}

/// Delete (disable) a client
pub async fn delete_client<S: StorageAdapter>(
    storage: &S,
    client_id: &str,
) -> Result<(), OAuthError> {
    // We don't actually delete, just disable
    let mut request = UpdateClientRequest {
        redirect_uris: None,
        allowed_grant_types: None,
        allowed_scopes: None,
        pkce_required: None,
        access_token_ttl: None,
        refresh_token_ttl: None,
        enabled: Some(false),
    };

    update_client(storage, client_id, request).await?;
    Ok(())
}

/// Rotate a client's secret
pub async fn rotate_client_secret<S: StorageAdapter>(
    storage: &S,
    client_id: &str,
) -> Result<String, OAuthError> {
    let key = ["client", client_id, "config"];

    let mut client = get_client(storage, client_id)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?
        .ok_or_else(|| OAuthError::InvalidClient(format!("Client '{}' not found", client_id)))?;

    if client.client_type != ClientType::Confidential {
        return Err(OAuthError::InvalidClient(
            "Cannot rotate secret for public client".to_string(),
        ));
    }

    // Generate new secret
    let secret = generate_client_secret();
    let hash = hash_client_secret(&secret)
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    client.client_secret_hash = Some(hash);
    client.updated_at = Utc::now();

    // Save updated client
    let value = serde_json::to_value(&client)
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;
    storage
        .set(&key, value, None)
        .await
        .map_err(|e| OAuthError::ServerError(e.to_string()))?;

    Ok(secret)
}
