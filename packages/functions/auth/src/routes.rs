//! HTTP route definitions for Irongate OAuth 2.0 server.
//!
//! Uses Axum for routing with Lambda integration.

use axum::{
    extract::Request,
    middleware,
    middleware::Next,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use url::form_urlencoded;

use crate::api::providers::password::{
    password_forgot_handler, password_login_handler, password_register_handler,
    password_reset_handler, password_verify_handler,
};
use crate::config::{AppState, Endpoint};
use crate::store::rate_limits::client_source_rate_limit_identifier;

/// Create the main Axum router with all routes
pub fn create_router(state: AppState) -> Router {
    let rate_limit_authorize = {
        let app = state.clone();
        middleware::from_fn(move |req: Request, next: Next| {
            let app = app.clone();
            async move {
                let client_id = extract_client_id_from_request(&req);
                let ip = crate::ratelimit::middleware::trusted_source_ip(
                    req.extensions(),
                    req.headers(),
                );
                let identifier =
                    client_source_rate_limit_identifier(client_id.as_deref(), ip.as_deref());

                match app
                    .store
                    .check_rate_limit(&app.config.rate_limit, Endpoint::Authorize, &identifier)
                    .await
                {
                    Ok(()) => next.run(req).await,
                    Err(err) => err.into_response(),
                }
            }
        })
    };

    let dev_mode = state.config.dev_mode;

    let cors_layer = if dev_mode {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
    };

    Router::new()
        // Well-known endpoints (public)
        .route(
            "/.well-known/oauth-authorization-server",
            get(crate::api::oauth::discovery::oauth_authorization_server),
        )
        .route(
            "/.well-known/openid-configuration",
            get(crate::api::oauth::discovery::openid_configuration),
        )
        .route(
            "/.well-known/jwks.json",
            get(crate::api::oauth::discovery::jwks),
        )
        // OAuth endpoints
        .route(
            "/authorize",
            get(crate::api::oauth::authorize::handle_authorize).route_layer(rate_limit_authorize),
        )
        .route("/token", post(crate::api::oauth::token::handle_token))
        .route(
            "/oauth/revoke",
            post(crate::api::oauth::revoke::handle_revoke),
        )
        .route(
            "/userinfo",
            get(crate::api::oauth::userinfo::handle_userinfo),
        )
        .route("/password/register", post(password_register_handler))
        .route("/password/verify", post(password_verify_handler))
        .route("/password/login", post(password_login_handler))
        .route("/password/forgot", post(password_forgot_handler))
        .route("/password/reset", post(password_reset_handler))
        .route(
            "/google/authorize",
            get(crate::api::providers::google::google_authorize_handler),
        )
        .route(
            "/google/callback",
            get(crate::api::providers::google::google_callback_handler),
        )
        .route(
            "/apple/authorize",
            get(crate::api::providers::apple::apple_authorize_handler),
        )
        .route(
            "/apple/callback",
            post(crate::api::providers::apple::apple_callback_handler),
        )
        // CORS must be outermost (added last = wraps everything)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_client_id_from_request(req: &Request) -> Option<String> {
    if let Some(auth) = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
    {
        if let Ok((client_id, _)) = crate::client::parse_basic_auth(Some(auth)) {
            return Some(client_id);
        }
    }

    if let Some(query) = req.uri().query() {
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            if key == "client_id" {
                let client_id = value.into_owned();
                if !client_id.is_empty() {
                    return Some(client_id);
                }
            }
        }
    }

    None
}
