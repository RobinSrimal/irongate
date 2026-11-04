use axum::body::{to_bytes, Body};
use axum::http::{
    header::{
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
        ACCESS_CONTROL_REQUEST_METHOD, LOCATION, ORIGIN,
    },
    Method, Request, StatusCode,
};
use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config, Endpoint, RateLimit};
use irongate::core::passwords::hash_password_for_storage;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::password::verify_password;
use irongate::crypto::signing::LocalEs256Signer;
use irongate::routes::create_router;
use irongate::storage::StorageAdapter;
use irongate::store::records::AuthorizeSessionRecord;
use irongate::store::AuthStore;
use irongate::store::IdentityProvider;
use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayV2httpRequestContext, ApiGatewayV2httpRequestContextHttpDescription,
};
use lambda_http::request::RequestContext;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

mod support;
use support::{NoopEmailSender, TestStorage};

fn write_client_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-route-client-config-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&path, contents).expect("write client config");
    path
}

fn runtime_with_public_client() -> Arc<RuntimeAuthConfig> {
    let client_config = r#"
[[clients]]
client_id = "web"
client_type = "spa"
redirect_uris = ["https://app.example.com/auth/callback"]
allowed_origins = ["https://app.example.com"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email"]
pkce_required = true
token_endpoint_auth_method = "none"
"#;
    let path = write_client_config(client_config);
    let signer = LocalEs256Signer::generate().expect("signer");
    let env = HashMap::from([
        (
            "AUTH_CLIENT_CONFIG_PATH".to_string(),
            path.display().to_string(),
        ),
        (
            "AUTH_HMAC_LOOKUP_SECRET".to_string(),
            "0123456789abcdef0123456789abcdef".to_string(),
        ),
        ("AUTH_SIGNING_MODE".to_string(), "local-es256".to_string()),
        ("AUTH_SIGNING_KEY_ID".to_string(), "test-key".to_string()),
        (
            "AUTH_SIGNING_PRIVATE_KEY_SECRET".to_string(),
            "AUTH_SIGNING_PRIVATE_KEY".to_string(),
        ),
        (
            "AUTH_SIGNING_PRIVATE_KEY".to_string(),
            signer.signing_key().private_key_pem.clone(),
        ),
        ("RESEND_API_KEY".to_string(), "re_test_key".to_string()),
        (
            "AUTH_EMAIL_FROM".to_string(),
            "Irongate <auth@example.com>".to_string(),
        ),
        (
            "AUTH_EMAIL_VERIFY_URL_BASE".to_string(),
            "https://app.example.com/auth/verify-email".to_string(),
        ),
        (
            "AUTH_EMAIL_RESET_URL_BASE".to_string(),
            "https://app.example.com/auth/reset-password".to_string(),
        ),
    ]);

    Arc::new(
        RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
            .expect("runtime config"),
    )
}

fn app_state() -> AppState {
    app_state_with_config(Config::dev())
}

fn app_state_with_config(config: Config) -> AppState {
    app_state_with_config_and_storage(config, TestStorage::new()).0
}

fn app_state_with_storage() -> (AppState, TestStorage) {
    app_state_with_config_and_storage(Config::dev(), TestStorage::new())
}

fn app_state_with_config_and_storage(
    config: Config,
    storage: TestStorage,
) -> (AppState, TestStorage) {
    let state = AppState {
        store: AuthStore::new(storage.clone()),
        audit: support::audit::emitter(irongate::config::audit::AuditLogMode::CloudWatch),
        config: Arc::new(config),
        runtime: runtime_with_public_client(),
        email_sender: Arc::new(NoopEmailSender::default()),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(irongate::providers::apple::ReqwestAppleOidcClient::new()),
    };
    (state, storage)
}

fn api_gateway_context(source_ip: &str) -> RequestContext {
    let mut context = ApiGatewayV2httpRequestContext::default();
    context.http = ApiGatewayV2httpRequestContextHttpDescription {
        source_ip: Some(source_ip.to_string()),
        ..Default::default()
    };
    RequestContext::ApiGatewayV2(context)
}

#[tokio::test]
async fn cors_allows_configured_browser_origin_without_wildcard() {
    let app = create_router(app_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header(ORIGIN, "https://app.example.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("grant_type=authorization_code&client_id=web"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&"https://app.example.com".parse().unwrap())
    );
    assert_ne!(
        response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&"*".parse().unwrap())
    );
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
        .is_none());
}

#[tokio::test]
async fn cors_rejects_unknown_origin() {
    let app = create_router(app_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header(ORIGIN, "https://evil.example.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("grant_type=authorization_code&client_id=web"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
}

#[tokio::test]
async fn cors_preflight_for_token_uses_configured_origin() {
    let app = create_router(app_state());
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/token")
                .header(ORIGIN, "https://app.example.com")
                .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&"https://app.example.com".parse().unwrap())
    );
    let methods = response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|value| value.to_str().ok())
        .expect("allow methods");
    assert!(methods.contains("POST"));
    let headers = response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_HEADERS)
        .and_then(|value| value.to_str().ok())
        .expect("allow headers");
    assert!(headers.to_ascii_lowercase().contains("content-type"));
}

#[tokio::test]
async fn cors_preflight_rejects_unknown_origin() {
    let app = create_router(app_state());
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/token")
                .header(ORIGIN, "https://evil.example.com")
                .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
}

#[tokio::test]
async fn authorize_uses_config_client_and_stores_hmac_session() {
    let (state, storage) = app_state_with_storage();
    let app = create_router(state);
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&scope=openid%20email&provider=password&nonce=nonce-123&code_challenge=challenge&code_challenge_method=S256";

    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("location header");
    assert!(location.starts_with("/password/login?session="));
    let raw_session = location
        .split_once("session=")
        .map(|(_, session)| session)
        .expect("session query");
    let sessions = storage
        .query_prefix(&["oauth:session"])
        .await
        .expect("query_prefix sessions");
    assert_eq!(sessions.len(), 1);
    assert!(!sessions[0].0.iter().any(|part| part.contains(raw_session)));
    assert_eq!(sessions[0].1["oidc_nonce"], "nonce-123");
}

#[tokio::test]
async fn authorize_rate_limit_uses_client_and_trusted_source_not_forwarded_headers() {
    let mut config = Config::dev();
    *config
        .rate_limit
        .policies
        .get_mut(&Endpoint::Authorize)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap() = RateLimit {
        requests: 5,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&scope=openid%20email&provider=password&nonce=nonce-123&code_challenge=challenge&code_challenge_method=S256";

    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("x-forwarded-for", "198.51.100.1")
                .header("x-real-ip", "198.51.100.2")
                .extension(api_gateway_context("203.0.113.44"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::Authorize.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(keys.contains("client_source:"));
    assert!(!keys.contains("203.0.113.44"));
    assert!(!keys.contains("198.51.100.1"));
    assert!(!keys.contains("198.51.100.2"));
}

#[tokio::test]
async fn token_rejects_client_credentials_before_issuing_tokens() {
    let app = create_router(app_state());
    let body = "grant_type=client_credentials&client_id=web";

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn public_client_token_rate_limit_is_scoped_by_trusted_source() {
    let mut config = Config::dev();
    *config
        .rate_limit
        .policies
        .get_mut(&Endpoint::Token)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap() = RateLimit {
        requests: 1,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let body = "grant_type=authorization_code&client_id=web&code=invalid&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&code_verifier=wrong";

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .extension(api_gateway_context("203.0.113.10"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::BAD_REQUEST);

    let second_same_source = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .extension(api_gateway_context("203.0.113.10"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_same_source.status(), StatusCode::TOO_MANY_REQUESTS);

    let different_source = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .extension(api_gateway_context("203.0.113.11"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(different_source.status(), StatusCode::BAD_REQUEST);

    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::Token.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(keys.contains("client_source:"));
    assert!(!keys.contains("203.0.113.10"));
    assert!(!keys.contains("203.0.113.11"));
}

#[tokio::test]
async fn public_bootstrap_route_is_not_mounted() {
    let app = create_router(app_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "unexpected status: {}",
        response.status()
    );
}

#[tokio::test]
async fn runtime_client_management_routes_are_not_mounted() {
    let app = create_router(app_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/clients")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "unexpected status: {}",
        response.status()
    );
}

#[tokio::test]
async fn legacy_dynamic_provider_routes_are_not_mounted() {
    let app = create_router(app_state());

    let get_authorize = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/legacy/authorize?session=raw-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_authorize.status(), StatusCode::NOT_FOUND);

    let get_callback = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/legacy/callback?code=abc&state=def")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_callback.status(), StatusCode::NOT_FOUND);

    let post_callback = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/legacy/callback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("session=raw-session"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_callback.status(), StatusCode::NOT_FOUND);
}

#[path = "api_routes/password.rs"]
mod password;

#[path = "api_routes/admission.rs"]
mod admission;
#[path = "support/rate_control.rs"]
mod rate_control;
#[path = "api_routes/rate_limits.rs"]
mod rate_limits;
#[path = "support/recovery.rs"]
#[allow(dead_code)]
mod recovery_fixture;
