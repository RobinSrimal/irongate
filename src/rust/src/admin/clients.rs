//! Client management endpoints.
//!
//! CRUD operations for OAuth clients via the Management API.

use axum::{
    extract::{Path, State},
    Json,
};

use crate::client::{
    self,
    Client, CreateClientRequest, CreateClientResponse, UpdateClientRequest,
};
use crate::config::AppState;
use crate::error::{IrongateError, OAuthError};
use crate::storage::StorageAdapter;

/// List all registered clients
pub async fn list_clients<S: StorageAdapter>(
    State(state): State<AppState<S>>,
) -> Result<Json<Vec<Client>>, IrongateError> {
    // TODO: Implement pagination
    let clients = state
        .storage
        .scan(&["client"])
        .await?
        .into_iter()
        .filter_map(|(_, value)| serde_json::from_value(value).ok())
        .collect();

    Ok(Json(clients))
}

/// Get a specific client by ID
pub async fn get_client<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Path(client_id): Path<String>,
) -> Result<Json<Client>, IrongateError> {
    let found = client::get_client(state.storage.as_ref(), &client_id)
        .await?
        .ok_or_else(|| OAuthError::InvalidClient(format!("Client '{}' not found", client_id)))?;

    Ok(Json(found))
}

/// Create a new client
pub async fn create_client<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Json(request): Json<CreateClientRequest>,
) -> Result<Json<CreateClientResponse>, IrongateError> {
    let response = client::create_client(state.storage.as_ref(), request).await?;
    Ok(Json(response))
}

/// Update an existing client
pub async fn update_client<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Path(client_id): Path<String>,
    Json(request): Json<UpdateClientRequest>,
) -> Result<Json<Client>, IrongateError> {
    let updated = client::update_client(state.storage.as_ref(), &client_id, request).await?;
    Ok(Json(updated))
}

/// Delete (disable) a client
pub async fn delete_client<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Path(client_id): Path<String>,
) -> Result<(), IrongateError> {
    client::delete_client(state.storage.as_ref(), &client_id).await?;
    Ok(())
}

/// Rotate a client's secret
pub async fn rotate_secret<S: StorageAdapter>(
    State(state): State<AppState<S>>,
    Path(client_id): Path<String>,
) -> Result<Json<serde_json::Value>, IrongateError> {
    let new_secret = client::rotate_client_secret(state.storage.as_ref(), &client_id).await?;
    Ok(Json(serde_json::json!({
        "client_secret": new_secret,
        "message": "Client secret rotated. Save this secret - it will not be shown again!"
    })))
}
