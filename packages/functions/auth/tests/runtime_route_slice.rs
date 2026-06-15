use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config, ProviderConfig};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::routes::create_router;
use irongate::store::AuthStore;
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
client_type = "public"
redirect_uris = ["https://app.example.com/auth/callback"]
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
    ]);

    Arc::new(
        RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
            .expect("runtime config"),
    )
}

fn app_state() -> AppState<TestStorage> {
    AppState {
        storage: Arc::new(TestStorage::new()),
        config: Arc::new(Config::dev()),
        runtime: runtime_with_public_client(),
        providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
        email_sender: Arc::new(NoopEmailSender::default()),
    }
}

#[tokio::test]
async fn authorize_uses_config_client_without_dynamodb_client_record() {
    let app = create_router(app_state());
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&code_challenge=challenge&code_challenge_method=S256";

    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
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
async fn password_register_route_returns_verification_required_without_tokens() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let app = create_router(state);
    let body = r#"{"email":"user@example.com","password":"correct horse battery staple"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("json response");

    assert_eq!(body["status"], "verification_required");
    assert!(body.get("delivery_id").is_none());
    assert!(body.get("authorization_code").is_none());
    assert!(body.get("access_token").is_none());

    let email_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::Email,
        "user@example.com",
    );
    let store = AuthStore::new(storage);
    let password_user = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get password user");

    assert!(password_user.is_some());
}

#[tokio::test]
async fn password_verify_route_returns_subject_without_tokens() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let store = AuthStore::new(storage);
    let email = "user@example.com";
    let token = "route-verification-token";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let verification_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        token,
    );

    store
        .create_unverified_password_user(&email_digest, email, "$argon2id$test-hash")
        .await
        .expect("create password user");
    store
        .create_email_verification(
            &verification_digest,
            &email_digest,
            Utc::now() + Duration::minutes(10),
        )
        .await
        .expect("create verification");

    let app = create_router(state);
    let body = r#"{"token":"route-verification-token"}"#;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/verify")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("json response");

    assert_eq!(body["status"], "verified");
    assert!(body["subject"]
        .as_str()
        .expect("subject")
        .starts_with("user_"));
    assert!(body.get("authorization_code").is_none());
    assert!(body.get("access_token").is_none());
}
