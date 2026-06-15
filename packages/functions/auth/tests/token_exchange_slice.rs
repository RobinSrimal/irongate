use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config, ProviderConfig};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::oauth::pkce::generate_challenge;
use irongate::oauth::well_known::build_authorization_server_metadata;
use irongate::routes::create_router;
use irongate::store::records::AuthorizationCodeRecord;
use irongate::store::{AuthStore, IdentityProvider};
use irongate::StorageAdapter;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde_json::json;
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
        "irongate-token-client-config-{}.toml",
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
            "slice-05-key".to_string(),
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

fn decode_token_claims(token: &str, runtime: &RuntimeAuthConfig, expected_audience: &str) -> Value {
    let header = decode_header(token).expect("token header");
    assert_eq!(header.kid.as_deref(), Some(runtime.signer.kid()));

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_issuer(&["https://auth.example.com"]);
    validation.set_audience(&[expected_audience]);
    let key = DecodingKey::from_ec_pem(runtime.signer.signing_key().public_key_pem.as_bytes())
        .expect("decoding key");

    decode::<Value>(token, &key, &validation)
        .expect("valid token")
        .claims
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

#[test]
fn metadata_advertises_refresh_after_revoke_route_exists() {
    let metadata = build_authorization_server_metadata("https://auth.example.com");
    let metadata_json = serde_json::to_value(&metadata).expect("metadata json");

    assert_eq!(
        metadata.grant_types_supported,
        vec![
            "authorization_code".to_string(),
            "refresh_token".to_string()
        ]
    );
    assert!(metadata
        .grant_types_supported
        .contains(&"refresh_token".to_string()));
    assert!(metadata
        .scopes_supported
        .contains(&"offline_access".to_string()));
    assert_eq!(
        metadata_json["revocation_endpoint"],
        "https://auth.example.com/oauth/revoke"
    );
    assert!(metadata_json.get("introspection_endpoint").is_none());
}

#[tokio::test]
async fn jwks_endpoint_uses_runtime_signer_without_signing_key_storage() {
    let state = app_state();
    let runtime_kid = state.runtime.signer.kid().to_string();
    let storage = state.storage.clone();
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("jwks json");
    assert_eq!(body["keys"][0]["kid"], runtime_kid);
    assert!(body["keys"][0].get("d").is_none());

    let stored_signing_keys = storage
        .scan(&["signing:key"])
        .await
        .expect("scan signing keys");
    assert!(stored_signing_keys.is_empty());
}

#[tokio::test]
async fn authorization_code_exchange_returns_runtime_signed_tokens_without_refresh() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let store = AuthStore::new((*storage).clone());
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
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
    let raw_code = "raw-slice-05-authorization-code";
    let verifier = "slice-05-verifier-with-enough-entropy";
    seed_authorization_code(
        &store,
        &runtime,
        raw_code,
        subject.as_str(),
        verifier,
        "openid email",
    )
    .await;

    let app = create_router(state);
    let response = exchange_code(app.clone(), raw_code, verifier).await;

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("token response");
    let access_token = body["access_token"].as_str().expect("access token");
    let id_token = body["id_token"].as_str().expect("id token");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], runtime.ttls.access_token_seconds);
    assert_eq!(body["scope"], "openid email");
    assert!(body.get("refresh_token").is_none());

    let access_claims = decode_token_claims(access_token, &runtime, "https://api.example.com");
    assert_eq!(access_claims["mode"], "access");
    assert_eq!(access_claims["sub"], subject.as_str());
    assert_eq!(access_claims["aud"], "https://api.example.com");
    assert_eq!(access_claims["scope"], "openid email");
    assert_eq!(access_claims["subject_type"], "user");
    assert_eq!(access_claims["properties"]["email"], "user@example.com");
    assert_eq!(access_claims["properties"]["email_verified"], true);

    let id_claims = decode_token_claims(id_token, &runtime, "web");
    assert_eq!(id_claims["mode"], "id");
    assert_eq!(id_claims["sub"], subject.as_str());
    assert_eq!(id_claims["aud"], "web");
    assert_eq!(id_claims["nonce"], "nonce-123");
    assert_eq!(id_claims["email"], "user@example.com");
    assert_eq!(id_claims["email_verified"], true);

    let code_records = storage.scan(&["oauth:code"]).await.expect("scan codes");
    assert!(code_records.is_empty());
    let signing_keys = storage
        .scan(&["signing:key"])
        .await
        .expect("scan signing keys");
    assert!(signing_keys.is_empty());

    let replay = exchange_code(app, raw_code, verifier).await;
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    let replay_body: Value = serde_json::from_slice(
        &to_bytes(replay.into_body(), 1024 * 1024)
            .await
            .expect("replay body"),
    )
    .expect("replay json");
    assert_eq!(replay_body["error"], "invalid_grant");
}

#[tokio::test]
async fn token_exchange_access_token_can_call_userinfo_but_id_token_cannot() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let store = AuthStore::new((*state.storage).clone());
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
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
    let raw_code = "raw-slice-05-userinfo-code";
    let verifier = "slice-05-userinfo-verifier";
    seed_authorization_code(
        &store,
        &runtime,
        raw_code,
        subject.as_str(),
        verifier,
        "openid email",
    )
    .await;

    let app = create_router(state);
    let token_response = exchange_code(app.clone(), raw_code, verifier).await;
    assert_eq!(token_response.status(), StatusCode::OK);
    let token_body: Value = serde_json::from_slice(
        &to_bytes(token_response.into_body(), 1024 * 1024)
            .await
            .expect("token body"),
    )
    .expect("token json");
    let access_token = token_body["access_token"].as_str().expect("access token");
    let id_token = token_body["id_token"].as_str().expect("id token");

    let userinfo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/userinfo")
                .header("authorization", format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(userinfo_response.status(), StatusCode::OK);
    let userinfo: Value = serde_json::from_slice(
        &to_bytes(userinfo_response.into_body(), 1024 * 1024)
            .await
            .expect("userinfo body"),
    )
    .expect("userinfo json");
    assert_eq!(userinfo["sub"], subject.as_str());
    assert_eq!(userinfo["type"], "user");
    assert_eq!(userinfo["email"], "user@example.com");
    assert_eq!(userinfo["email_verified"], true);
    assert!(userinfo.get("properties").is_none());

    let id_userinfo_response = app
        .oneshot(
            Request::builder()
                .uri("/userinfo")
                .header("authorization", format!("Bearer {id_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(id_userinfo_response.status(), StatusCode::BAD_REQUEST);
    let id_userinfo: Value = serde_json::from_slice(
        &to_bytes(id_userinfo_response.into_body(), 1024 * 1024)
            .await
            .expect("id userinfo body"),
    )
    .expect("id userinfo json");
    assert_eq!(id_userinfo["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_exchange_with_offline_access_returns_refresh_token() {
    let state = app_state();
    let runtime = state.runtime.clone();
    let store = AuthStore::new((*state.storage).clone());
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
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
    let raw_code = "raw-slice-05-offline-code";
    let verifier = "slice-05-offline-verifier";
    seed_authorization_code(
        &store,
        &runtime,
        raw_code,
        subject.as_str(),
        verifier,
        "openid offline_access",
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
    assert!(body["refresh_token"].as_str().is_some());
    assert_eq!(body["scope"], "openid offline_access");
}
