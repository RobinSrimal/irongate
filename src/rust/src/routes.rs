//! HTTP route definitions for Irongate OAuth 2.0 server.
//!
//! Uses Axum for routing with Lambda integration.

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::storage::DynamoStorage;

/// Create the main Axum router with all routes
pub fn create_router(_storage: DynamoStorage) -> Router {
    Router::new()
        // Well-known endpoints (public)
        .route(
            "/.well-known/oauth-authorization-server",
            get(well_known_handler),
        )
        .route("/.well-known/jwks.json", get(jwks_handler))
        // OAuth endpoints
        .route("/authorize", get(authorize_handler))
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        // Admin endpoints (authenticated)
        .route("/admin/bootstrap", post(bootstrap_handler))
        .route("/admin/clients", get(list_clients_handler))
        .route("/admin/clients", post(create_client_handler))
        .route("/admin/clients/:id", get(get_client_handler))
        .route("/admin/clients/:id", axum::routing::put(update_client_handler))
        .route("/admin/clients/:id", axum::routing::delete(delete_client_handler))
        .route("/admin/clients/:id/rotate-secret", post(rotate_secret_handler))
        .route("/admin/tokens/revoke", post(revoke_tokens_handler))
        // Provider routes (mounted dynamically)
        .route("/:provider/authorize", get(provider_authorize_handler))
        .route("/:provider/callback", get(provider_callback_handler))
        .route("/:provider/callback", post(provider_callback_handler))
        // Add tracing
        .layer(TraceLayer::new_for_http())
}

// Placeholder handlers - will be implemented with proper state management
async fn well_known_handler() -> &'static str {
    todo!("OAuth authorization server metadata")
}

async fn jwks_handler() -> &'static str {
    todo!("JWKS endpoint")
}

async fn authorize_handler() -> &'static str {
    todo!("Authorization endpoint")
}

async fn token_handler() -> &'static str {
    todo!("Token endpoint")
}

async fn userinfo_handler() -> &'static str {
    todo!("UserInfo endpoint")
}

async fn bootstrap_handler() -> &'static str {
    todo!("Bootstrap admin key")
}

async fn list_clients_handler() -> &'static str {
    todo!("List clients")
}

async fn create_client_handler() -> &'static str {
    todo!("Create client")
}

async fn get_client_handler() -> &'static str {
    todo!("Get client")
}

async fn update_client_handler() -> &'static str {
    todo!("Update client")
}

async fn delete_client_handler() -> &'static str {
    todo!("Delete client")
}

async fn rotate_secret_handler() -> &'static str {
    todo!("Rotate client secret")
}

async fn revoke_tokens_handler() -> &'static str {
    todo!("Revoke tokens")
}

async fn provider_authorize_handler() -> &'static str {
    todo!("Provider authorize")
}

async fn provider_callback_handler() -> &'static str {
    todo!("Provider callback")
}
