use axum::body::{to_bytes, Body};
use axum::http::{header::LOCATION, Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config, Endpoint, ProviderConfig, RateLimit};
use irongate::core::passwords::hash_password_for_storage;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::password::verify_password;
use irongate::crypto::signing::LocalEs256Signer;
use irongate::routes::create_router;
use irongate::store::records::AuthorizeSessionRecord;
use irongate::store::AuthStore;
use irongate::store::IdentityProvider;
use irongate::StorageAdapter;
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

fn app_state() -> AppState<TestStorage> {
    app_state_with_config(Config::dev())
}

fn app_state_with_config(config: Config) -> AppState<TestStorage> {
    AppState {
        storage: Arc::new(TestStorage::new()),
        config: Arc::new(config),
        runtime: runtime_with_public_client(),
        providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
        email_sender: Arc::new(NoopEmailSender::default()),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(irongate::providers::apple::ReqwestAppleOidcClient::new()),
    }
}

#[tokio::test]
async fn authorize_uses_config_client_and_stores_hmac_session() {
    let state = app_state();
    let storage = state.storage.clone();
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
        .scan(&["oauth:session"])
        .await
        .expect("scan sessions");
    assert_eq!(sessions.len(), 1);
    assert!(!sessions[0].0.iter().any(|part| part.contains(raw_session)));
    assert_eq!(sessions[0].1["oidc_nonce"], "nonce-123");
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

#[tokio::test]
async fn password_forgot_route_returns_generic_success_without_tokens() {
    let state = app_state();
    let storage = state.storage.clone();
    let app = create_router(state);
    let body = r#"{"email":"unknown@example.com"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/forgot")
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
    assert_eq!(body["status"], "reset_email_sent");
    assert!(body.get("code").is_none());
    assert!(body.get("access_token").is_none());
    assert!(body.get("refresh_token").is_none());
    assert!(body.get("id_token").is_none());

    let reset_records = storage.scan(&["password:reset"]).await.expect("scan resets");
    assert!(reset_records.is_empty());
}

#[tokio::test]
async fn password_reset_route_updates_password_without_tokens() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let store = AuthStore::new(storage.clone());
    let email = "user@example.com";
    let old_password = "correct horse battery staple";
    let new_password = "new correct horse battery staple";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let old_hash = hash_password_for_storage(old_password).expect("old hash");
    store
        .create_unverified_password_user(&email_digest, email, &old_hash)
        .await
        .expect("create password user");
    let subject = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            serde_json::json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify user");
    let reset_token = "route-reset-token";
    let reset_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordReset,
        reset_token,
    );
    store
        .create_password_reset(
            &reset_digest,
            &email_digest,
            subject.as_str(),
            Utc::now() + Duration::minutes(10),
        )
        .await
        .expect("create reset");

    let app = create_router(state);
    let body = r#"{"token":"route-reset-token","new_password":"new correct horse battery staple"}"#;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/reset")
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
    assert_eq!(body["status"], "password_reset");
    assert!(body.get("code").is_none());
    assert!(body.get("access_token").is_none());
    assert!(body.get("refresh_token").is_none());
    assert!(body.get("id_token").is_none());

    let updated = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get user")
        .expect("user");
    assert!(verify_password(new_password, &updated.password_hash));
    assert!(!verify_password(old_password, &updated.password_hash));
}

#[tokio::test]
async fn password_login_route_redirects_with_authorization_code() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let store = AuthStore::new(storage.clone());
    let email = "user@example.com";
    let password = "correct horse battery staple";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let password_hash = hash_password_for_storage(password).expect("hash password");
    store
        .create_unverified_password_user(&email_digest, email, &password_hash)
        .await
        .expect("create password user");
    store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            serde_json::json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify password user");

    let raw_session = "route-login-session";
    let session_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    store
        .create_authorize_session(
            &session_digest,
            AuthorizeSessionRecord {
                client_id: "web".to_string(),
                redirect_uri: "https://app.example.com/auth/callback".to_string(),
                state: Some("abc".to_string()),
                scope: "openid email".to_string(),
                oidc_nonce: None,
                code_challenge: Some("challenge".to_string()),
                code_challenge_method: Some("S256".to_string()),
                selected_provider: Some("password".to_string()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .expect("create session");

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "session={raw_session}&email=user%40example.com&password=correct+horse+battery+staple"
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("location header");
    assert!(location.starts_with("https://app.example.com/auth/callback?"));
    assert!(location.contains("code="));
    assert!(location.contains("state=abc"));
    assert!(!location.contains("access_token"));
    assert!(!location.contains("refresh_token"));
    assert!(!location.contains("id_token"));
}

#[tokio::test]
async fn password_register_route_is_rate_limited_without_raw_email_keys() {
    let mut config = Config::dev();
    config.rate_limit.limits.insert(
        Endpoint::PasswordRegister,
        RateLimit {
            requests: 1,
            window_seconds: 60,
        },
    );
    let state = app_state_with_config(config);
    let storage = state.storage.clone();
    let app = create_router(state);
    let body = r#"{"email":"user@example.com","password":"correct horse battery staple"}"#;

    let first = app
        .clone()
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
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
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
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .scan(&["ratelimit"])
        .await
        .expect("scan rate limits");
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains("user@example.com"));
    assert!(!keys.contains("correct horse battery staple"));
}

#[tokio::test]
async fn password_verify_route_is_rate_limited_without_raw_token_keys() {
    let mut config = Config::dev();
    config.rate_limit.limits.insert(
        Endpoint::PasswordVerify,
        RateLimit {
            requests: 1,
            window_seconds: 60,
        },
    );
    let state = app_state_with_config(config);
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let store = AuthStore::new(storage.clone());
    let email_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::Email,
        "user@example.com",
    );
    let token = "route-verification-token";
    let verification_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        token,
    );
    store
        .create_unverified_password_user(&email_digest, "user@example.com", "$argon2id$test-hash")
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
    let first = app
        .clone()
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
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
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
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .scan(&["ratelimit"])
        .await
        .expect("scan rate limits");
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains(token));
}

#[tokio::test]
async fn password_login_route_is_rate_limited_without_raw_email_or_session_keys() {
    let mut config = Config::dev();
    config.rate_limit.limits.insert(
        Endpoint::PasswordLogin,
        RateLimit {
            requests: 1,
            window_seconds: 60,
        },
    );
    let state = app_state_with_config(config);
    let storage = state.storage.clone();
    let app = create_router(state);
    let body = "session=raw-login-session&email=user%40example.com&password=wrong";

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(first.status(), StatusCode::TOO_MANY_REQUESTS);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .scan(&["ratelimit"])
        .await
        .expect("scan rate limits");
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains("user@example.com"));
    assert!(!keys.contains("raw-login-session"));
    assert!(!keys.contains("wrong"));
}

#[tokio::test]
async fn password_forgot_route_is_rate_limited_without_raw_email_keys() {
    let mut config = Config::dev();
    config.rate_limit.limits.insert(
        Endpoint::PasswordResetRequest,
        RateLimit {
            requests: 1,
            window_seconds: 60,
        },
    );
    let state = app_state_with_config(config);
    let storage = state.storage.clone();
    let app = create_router(state);
    let body = r#"{"email":"user@example.com"}"#;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/forgot")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/forgot")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .scan(&["ratelimit"])
        .await
        .expect("scan rate limits");
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains("user@example.com"));
}

#[tokio::test]
async fn password_reset_route_is_rate_limited_without_raw_token_or_password_keys() {
    let mut config = Config::dev();
    config.rate_limit.limits.insert(
        Endpoint::PasswordResetComplete,
        RateLimit {
            requests: 1,
            window_seconds: 60,
        },
    );
    let state = app_state_with_config(config);
    let app = create_router(state);
    let body = r#"{"token":"route-reset-token","new_password":"new correct horse battery staple"}"#;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/reset")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::BAD_REQUEST);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/reset")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}
