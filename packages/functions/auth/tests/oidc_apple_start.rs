use axum::body::Body;
use axum::http::{header::LOCATION, Request, StatusCode};
use chrono::Duration;
use chrono::{TimeZone, Utc};
use irongate::config::apple::{AppleConfig, APPLE_AUDIENCE};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::providers::apple::{
    build_apple_authorization_url, generate_apple_client_secret, AppleAuthorizeInput,
};
use irongate::routes::create_router;
use irongate::storage::StorageAdapter;
use irongate::store::records::AuthorizeSessionRecord;
use irongate::store::AuthStore;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;
use url::Url;

mod support;
use support::{NoopEmailSender, TestStorage};

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn apple_config() -> (AppleConfig, LocalEs256Signer) {
    let signer = LocalEs256Signer::generate().expect("apple signer");
    let private_key = signer.signing_key().private_key_pem.clone();
    let config = AppleConfig::from_values(
        Some("com.example.web"),
        Some("TEAMID1234"),
        Some("KEYID12345"),
        Some("AUTH_APPLE_PRIVATE_KEY"),
        Some(3_600),
        |name| (name == "AUTH_APPLE_PRIVATE_KEY").then(|| private_key.clone()),
    )
    .expect("apple config")
    .expect("apple enabled");

    (config, signer)
}

fn write_client_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-apple-client-config-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&path, contents).expect("write client config");
    path
}

fn runtime_with_apple_config(apple_enabled: bool) -> Arc<RuntimeAuthConfig> {
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
    let apple_signer = LocalEs256Signer::generate().expect("apple signer");
    let mut env = HashMap::from([
        (
            "AUTH_CLIENT_CONFIG_PATH".to_string(),
            path.display().to_string(),
        ),
        (
            "AUTH_HMAC_LOOKUP_SECRET".to_string(),
            String::from_utf8(LOOKUP_SECRET.to_vec()).expect("lookup secret"),
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

    if apple_enabled {
        env.insert(
            "AUTH_APPLE_CLIENT_ID".to_string(),
            "com.example.web".to_string(),
        );
        env.insert("AUTH_APPLE_TEAM_ID".to_string(), "TEAMID1234".to_string());
        env.insert("AUTH_APPLE_KEY_ID".to_string(), "KEYID12345".to_string());
        env.insert(
            "AUTH_APPLE_PRIVATE_KEY_SECRET".to_string(),
            "AUTH_APPLE_PRIVATE_KEY".to_string(),
        );
        env.insert(
            "AUTH_APPLE_PRIVATE_KEY".to_string(),
            apple_signer.signing_key().private_key_pem.clone(),
        );
    }

    Arc::new(
        RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
            .expect("runtime config"),
    )
}

fn apple_app_state(apple_enabled: bool) -> AppState {
    apple_app_state_with_storage(apple_enabled).0
}

fn apple_app_state_with_storage(apple_enabled: bool) -> (AppState, TestStorage) {
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".to_string());
    let storage = TestStorage::new();
    let state = AppState {
        store: AuthStore::new(storage.clone()),
        config: Arc::new(config),
        runtime: runtime_with_apple_config(apple_enabled),
        email_sender: Arc::new(NoopEmailSender::default()),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(irongate::providers::apple::ReqwestAppleOidcClient::new()),
    };
    (state, storage)
}

#[derive(Debug, Deserialize)]
struct AppleClientSecretClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: i64,
    exp: i64,
}

#[test]
fn apple_client_secret_uses_es256_header_and_apple_claims() {
    let (apple, signer) = apple_config();
    let now = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();

    let client_secret =
        generate_apple_client_secret(&apple, now).expect("generate apple client secret");
    let header = decode_header(&client_secret).expect("client secret header");
    assert_eq!(header.alg, Algorithm::ES256);
    assert_eq!(header.kid.as_deref(), Some("KEYID12345"));

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_audience(&[APPLE_AUDIENCE]);
    validation.validate_exp = false;
    let decoded = decode::<AppleClientSecretClaims>(
        &client_secret,
        &DecodingKey::from_ec_pem(signer.signing_key().public_key_pem.as_bytes())
            .expect("apple public key"),
        &validation,
    )
    .expect("decode client secret");

    assert_eq!(decoded.claims.iss, "TEAMID1234");
    assert_eq!(decoded.claims.sub, "com.example.web");
    assert_eq!(decoded.claims.aud, APPLE_AUDIENCE);
    assert_eq!(decoded.claims.iat, now.timestamp());
    assert_eq!(decoded.claims.exp, now.timestamp() + 3_600);
}

#[test]
fn apple_authorization_url_contains_state_nonce_pkce_and_form_post_without_secrets() {
    let (apple, _) = apple_config();

    let url = build_apple_authorization_url(AppleAuthorizeInput {
        config: &apple,
        redirect_uri: "https://auth.example.com/apple/callback",
        state: "raw-provider-state",
        nonce: "raw-provider-nonce",
        pkce_challenge: "provider-pkce-challenge",
    });

    let parsed = Url::parse(&url).expect("apple authorize url");
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("appleid.apple.com"));
    assert_eq!(parsed.path(), "/auth/authorize");

    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("com.example.web")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("https://auth.example.com/apple/callback")
    );
    assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
    assert_eq!(
        query.get("response_mode").map(String::as_str),
        Some("form_post")
    );
    assert_eq!(query.get("scope").map(String::as_str), Some("name email"));
    assert_eq!(
        query.get("state").map(String::as_str),
        Some("raw-provider-state")
    );
    assert_eq!(
        query.get("nonce").map(String::as_str),
        Some("raw-provider-nonce")
    );
    assert_eq!(
        query.get("code_challenge").map(String::as_str),
        Some("provider-pkce-challenge")
    );
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert!(!url.contains("BEGIN PRIVATE KEY"));
}

#[tokio::test]
async fn authorize_rejects_apple_provider_when_apple_is_disabled() {
    let app = create_router(apple_app_state(false));
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&scope=openid%20email&provider=apple&nonce=client-nonce&code_challenge=challenge&code_challenge_method=S256";

    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn authorize_accepts_apple_provider_and_redirects_to_apple_start() {
    let (state, storage) = apple_app_state_with_storage(true);
    let app = create_router(state);
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&scope=openid%20email&provider=apple&nonce=client-nonce&code_challenge=challenge&code_challenge_method=S256";

    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("location");
    assert!(location.starts_with("/apple/authorize?session="));
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
    assert_eq!(sessions[0].1["selected_provider"], "apple");
    assert_eq!(sessions[0].1["oidc_nonce"], "client-nonce");
}

#[tokio::test]
async fn apple_authorize_route_creates_provider_state_and_redirects_to_apple() {
    let (state, storage) = apple_app_state_with_storage(true);
    let runtime = state.runtime.clone();
    let store = AuthStore::new(storage.clone());
    let raw_session = "raw-apple-authorize-session";
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
                state: Some("client-state".to_string()),
                scope: "openid email".to_string(),
                oidc_nonce: Some("client-nonce".to_string()),
                code_challenge: Some("client-pkce-challenge".to_string()),
                code_challenge_method: Some("S256".to_string()),
                selected_provider: Some("apple".to_string()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .expect("create authorize session");

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/apple/authorize?session={raw_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("location");
    let parsed = Url::parse(location).expect("apple redirect url");
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("appleid.apple.com"));
    assert_eq!(parsed.path(), "/auth/authorize");

    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("com.example.web")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("https://auth.example.com/apple/callback")
    );
    assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
    assert_eq!(
        query.get("response_mode").map(String::as_str),
        Some("form_post")
    );
    assert_eq!(query.get("scope").map(String::as_str), Some("name email"));
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    let raw_provider_state = query.get("state").expect("provider state");
    assert!(!raw_provider_state.is_empty());
    assert!(query.get("nonce").is_some());
    assert!(query.get("code_challenge").is_some());
    assert!(!location.contains("BEGIN PRIVATE KEY"));

    let provider_states = storage
        .scan(&["provider:state"])
        .await
        .expect("scan provider state");
    assert_eq!(provider_states.len(), 1);
    let debug = format!("{provider_states:?}");
    assert!(!debug.contains(raw_provider_state));
    assert!(!debug.contains(raw_session));
    assert_eq!(
        provider_states[0].1["session_lookup_digest"],
        serde_json::Value::String(session_digest)
    );
    assert_eq!(provider_states[0].1["provider"], "apple");
}

#[tokio::test]
async fn apple_authorize_route_rejects_non_apple_authorize_session() {
    let (state, storage) = apple_app_state_with_storage(true);
    let runtime = state.runtime.clone();
    let store = AuthStore::new(storage);
    let raw_session = "raw-password-authorize-session";
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
                state: Some("client-state".to_string()),
                scope: "openid email".to_string(),
                oidc_nonce: Some("client-nonce".to_string()),
                code_challenge: Some("client-pkce-challenge".to_string()),
                code_challenge_method: Some("S256".to_string()),
                selected_provider: Some("password".to_string()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .expect("create authorize session");

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/apple/authorize?session={raw_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
