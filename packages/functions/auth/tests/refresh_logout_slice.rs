use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config, ProviderConfig};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::oauth::pkce::generate_challenge;
use irongate::routes::create_router;
use irongate::store::keys::StoreKey;
use irongate::store::records::{
    AuthorizationCodeRecord, RefreshTokenFamilyRecord, RefreshTokenRecord,
};
use irongate::store::refresh::{
    CreateRefreshTokenInput, RefreshTokenStoreError, RevokeRefreshTokenOutcome,
};
use irongate::store::{AuthStore, IdentityProvider};
use irongate::StorageAdapter;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

mod support;
use support::{NoopEmailSender, TestStorage};

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn write_client_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-refresh-client-config-{}.toml",
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
allowed_scopes = ["openid", "profile", "email", "offline_access"]
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
        (
            "AUTH_SIGNING_KEY_ID".to_string(),
            "slice-06-key".to_string(),
        ),
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
        (
            "AUTH_ACCESS_TOKEN_AUDIENCE".to_string(),
            "https://api.example.com".to_string(),
        ),
    ]);

    Arc::new(
        RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
            .expect("runtime config"),
    )
}

fn app_state() -> AppState<TestStorage> {
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".to_string());
    AppState {
        storage: Arc::new(TestStorage::new()),
        config: Arc::new(config),
        runtime: runtime_with_public_client(),
        providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
        email_sender: Arc::new(NoopEmailSender),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(irongate::providers::apple::ReqwestAppleOidcClient::new()),
    }
}

async fn seed_authorization_code(
    store: &AuthStore<TestStorage>,
    runtime: &RuntimeAuthConfig,
    raw_code: &str,
    subject: &str,
    verifier: &str,
    scope: &str,
) {
    let code_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizationCode,
        raw_code,
    );
    store
        .create_authorization_code(
            &code_digest,
            AuthorizationCodeRecord {
                client_id: "web".to_string(),
                redirect_uri: "https://app.example.com/auth/callback".to_string(),
                subject: subject.to_string(),
                subject_type: "user".to_string(),
                properties: json!({
                    "email": "user@example.com",
                    "email_verified": true,
                    "provider": "password"
                }),
                code_challenge: Some(generate_challenge(verifier)),
                code_challenge_method: Some("S256".to_string()),
                scope: scope.to_string(),
                oidc_nonce: Some("nonce-123".to_string()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(5),
            },
        )
        .await
        .expect("create authorization code");
}

async fn seed_account_with_code(
    state: &AppState<TestStorage>,
    raw_code: &str,
    verifier: &str,
    scope: &str,
) -> String {
    let store = AuthStore::new((*state.storage).clone());
    let identity_digest = lookup_digest(
        state.runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        "user@example.com",
    );
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            &identity_digest,
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");
    seed_authorization_code(
        &store,
        &state.runtime,
        raw_code,
        subject.as_str(),
        verifier,
        scope,
    )
    .await;
    subject.as_str().to_string()
}

async fn exchange_code(
    app: axum::Router,
    raw_code: &str,
    verifier: &str,
) -> axum::response::Response {
    let body = serde_urlencoded::to_string([
        ("grant_type", "authorization_code"),
        ("client_id", "web"),
        ("code", raw_code),
        ("redirect_uri", "https://app.example.com/auth/callback"),
        ("code_verifier", verifier),
    ])
    .expect("form body");

    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn refresh_token(app: axum::Router, token: &str) -> axum::response::Response {
    let body = serde_urlencoded::to_string([
        ("grant_type", "refresh_token"),
        ("client_id", "web"),
        ("refresh_token", token),
    ])
    .expect("form body");

    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn revoke_refresh_token(app: axum::Router, token: &str) -> axum::response::Response {
    let body = serde_urlencoded::to_string([
        ("client_id", "web"),
        ("token", token),
        ("token_type_hint", "refresh_token"),
    ])
    .expect("form body");

    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/oauth/revoke")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn create_refresh_token_uses_hmac_keys_and_no_raw_token_key() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let created = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    let expected_digest = lookup_digest(
        LOOKUP_SECRET,
        LookupFamily::RefreshToken,
        &created.raw_token,
    );
    assert_eq!(created.refresh_digest, expected_digest);

    let key = StoreKey::refresh_token(&created.refresh_digest);
    assert_ne!(key.sk(), created.raw_token);
    let stored = storage
        .get(&[key.pk(), key.sk()])
        .await
        .expect("get refresh record")
        .expect("refresh record");
    let record: RefreshTokenRecord = serde_json::from_value(stored).expect("refresh record json");
    assert_eq!(record.refresh_digest, created.refresh_digest);
    assert_eq!(record.family_id, created.family_id);
    assert_eq!(record.client_id, "web");
    assert_eq!(record.subject, subject.as_str());
    assert_eq!(record.scope, "openid email offline_access");
    assert!(record.revoked_at.is_none());

    let raw_key_record = storage
        .get(&["oauth:refresh", &created.raw_token])
        .await
        .expect("raw token lookup");
    assert!(raw_key_record.is_none());

    let family_key = StoreKey::refresh_family(&created.family_id);
    let family_value = storage
        .get(&[family_key.pk(), family_key.sk()])
        .await
        .expect("get refresh family")
        .expect("refresh family");
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(family_value).expect("refresh family json");
    assert_eq!(family.current_refresh_digest, created.refresh_digest);
    assert!(family.revoked_at.is_none());
}

#[tokio::test]
async fn rotate_refresh_token_replaces_once_and_revokes_family_on_reuse() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let first = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    let rotated = store
        .rotate_refresh_token(
            LOOKUP_SECRET,
            &first.raw_token,
            "web",
            Utc::now() + Duration::days(30),
        )
        .await
        .expect("rotate refresh token");

    assert_ne!(rotated.raw_token, first.raw_token);
    assert_ne!(rotated.refresh_digest, first.refresh_digest);
    assert_eq!(rotated.family_id, first.family_id);
    assert_eq!(rotated.subject, subject.as_str());
    assert_eq!(rotated.scope, "openid email offline_access");

    let first_key = StoreKey::refresh_token(&first.refresh_digest);
    let first_value = storage
        .get(&[first_key.pk(), first_key.sk()])
        .await
        .expect("get first refresh")
        .expect("first refresh");
    let first_record: RefreshTokenRecord =
        serde_json::from_value(first_value).expect("first refresh json");
    assert_eq!(first_record.replaced_by.as_deref(), Some(rotated.refresh_digest.as_str()));
    assert!(first_record.revoked_at.is_none());

    let reuse = store
        .rotate_refresh_token(
            LOOKUP_SECRET,
            &first.raw_token,
            "web",
            Utc::now() + Duration::days(30),
        )
        .await;
    assert!(matches!(reuse, Err(RefreshTokenStoreError::ReuseDetected)));

    let family_key = StoreKey::refresh_family(&first.family_id);
    let family_value = storage
        .get(&[family_key.pk(), family_key.sk()])
        .await
        .expect("get family")
        .expect("family");
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(family_value).expect("family json");
    assert!(family.revoked_at.is_some());
}

#[tokio::test]
async fn revoke_refresh_token_family_is_idempotent_and_client_bound() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let created = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    let wrong_client = store
        .revoke_refresh_token_family(LOOKUP_SECRET, &created.raw_token, "mobile")
        .await
        .expect("wrong-client revoke");
    assert_eq!(wrong_client, RevokeRefreshTokenOutcome::NotFound);

    let revoked = store
        .revoke_refresh_token_family(LOOKUP_SECRET, &created.raw_token, "web")
        .await
        .expect("revoke");
    assert_eq!(revoked, RevokeRefreshTokenOutcome::Revoked);

    let again = store
        .revoke_refresh_token_family(LOOKUP_SECRET, &created.raw_token, "web")
        .await
        .expect("revoke again");
    assert_eq!(again, RevokeRefreshTokenOutcome::AlreadyRevoked);

    let rotate = store
        .rotate_refresh_token(
            LOOKUP_SECRET,
            &created.raw_token,
            "web",
            Utc::now() + Duration::days(30),
        )
        .await;
    assert!(matches!(rotate, Err(RefreshTokenStoreError::Invalid)));
}

#[tokio::test]
async fn authorization_code_exchange_with_offline_access_returns_digest_stored_refresh_token() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let raw_code = "raw-slice-06-authorization-code";
    let verifier = "slice-06-verifier-with-enough-entropy";
    let subject = seed_account_with_code(
        &state,
        raw_code,
        verifier,
        "openid email offline_access",
    )
    .await;

    let response = exchange_code(create_router(state), raw_code, verifier).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("response body"),
    )
    .expect("token json");
    let refresh_token = body["refresh_token"].as_str().expect("refresh token");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], runtime.ttls.access_token_seconds);
    assert_eq!(body["scope"], "openid email offline_access");
    assert!(body["access_token"].as_str().is_some());
    assert!(body["id_token"].as_str().is_some());

    let refresh_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::RefreshToken,
        refresh_token,
    );
    let refresh_key = StoreKey::refresh_token(&refresh_digest);
    let stored = storage
        .get(&[refresh_key.pk(), refresh_key.sk()])
        .await
        .expect("get refresh record")
        .expect("refresh record");
    let record: RefreshTokenRecord = serde_json::from_value(stored).expect("refresh record json");
    assert_eq!(record.refresh_digest, refresh_digest);
    assert_eq!(record.subject, subject);
    assert_eq!(record.scope, "openid email offline_access");

    let raw_lookup = storage
        .get(&["oauth:refresh", refresh_token])
        .await
        .expect("raw refresh lookup");
    assert!(raw_lookup.is_none());
}

#[tokio::test]
async fn refresh_grant_rotates_once_and_reuse_revokes_family() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let raw_code = "raw-slice-06-rotation-code";
    let verifier = "slice-06-rotation-verifier";
    let subject = seed_account_with_code(
        &state,
        raw_code,
        verifier,
        "openid email offline_access",
    )
    .await;
    let app = create_router(state);
    let exchange = exchange_code(app.clone(), raw_code, verifier).await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let exchange_body: Value = serde_json::from_slice(
        &to_bytes(exchange.into_body(), 1024 * 1024)
            .await
            .expect("exchange body"),
    )
    .expect("exchange json");
    let first_refresh = exchange_body["refresh_token"]
        .as_str()
        .expect("first refresh")
        .to_string();
    let first_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::RefreshToken,
        &first_refresh,
    );

    let rotated = refresh_token(app.clone(), &first_refresh).await;

    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated_body: Value = serde_json::from_slice(
        &to_bytes(rotated.into_body(), 1024 * 1024)
            .await
            .expect("rotated body"),
    )
    .expect("rotated json");
    let second_refresh = rotated_body["refresh_token"]
        .as_str()
        .expect("second refresh")
        .to_string();
    assert_ne!(second_refresh, first_refresh);
    assert!(rotated_body["access_token"].as_str().is_some());
    assert!(rotated_body.get("id_token").is_none());
    assert_eq!(rotated_body["scope"], "openid email offline_access");

    let first_key = StoreKey::refresh_token(&first_digest);
    let first_value = storage
        .get(&[first_key.pk(), first_key.sk()])
        .await
        .expect("get first refresh")
        .expect("first refresh");
    let first_record: RefreshTokenRecord =
        serde_json::from_value(first_value).expect("first refresh json");
    assert_eq!(first_record.subject, subject);
    assert!(first_record.replaced_by.is_some());

    let replay = refresh_token(app.clone(), &first_refresh).await;
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    let replay_body: Value = serde_json::from_slice(
        &to_bytes(replay.into_body(), 1024 * 1024)
            .await
            .expect("replay body"),
    )
    .expect("replay json");
    assert_eq!(replay_body["error"], "invalid_grant");

    let second_attempt_after_reuse = refresh_token(app, &second_refresh).await;
    assert_eq!(second_attempt_after_reuse.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_revoke_is_idempotent_logout_for_refresh_token_family() {
    let state = app_state();
    let raw_code = "raw-slice-06-revoke-code";
    let verifier = "slice-06-revoke-verifier";
    seed_account_with_code(
        &state,
        raw_code,
        verifier,
        "openid email offline_access",
    )
    .await;
    let app = create_router(state);
    let exchange = exchange_code(app.clone(), raw_code, verifier).await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let exchange_body: Value = serde_json::from_slice(
        &to_bytes(exchange.into_body(), 1024 * 1024)
            .await
            .expect("exchange body"),
    )
    .expect("exchange json");
    let refresh = exchange_body["refresh_token"]
        .as_str()
        .expect("refresh token")
        .to_string();

    let revoked = revoke_refresh_token(app.clone(), &refresh).await;
    assert_eq!(revoked.status(), StatusCode::OK);

    let revoked_again = revoke_refresh_token(app.clone(), &refresh).await;
    assert_eq!(revoked_again.status(), StatusCode::OK);

    let refresh_after_logout = refresh_token(app, &refresh).await;
    assert_eq!(refresh_after_logout.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(
        &to_bytes(refresh_after_logout.into_body(), 1024 * 1024)
            .await
            .expect("refresh body"),
    )
    .expect("refresh json");
    assert_eq!(body["error"], "invalid_grant");
}
