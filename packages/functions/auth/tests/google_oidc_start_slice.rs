use axum::body::Body;
use axum::http::{header::LOCATION, Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::google::GoogleConfig;
use irongate::config::{AppState, Config, ProviderConfig};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::providers::google::{build_google_authorization_url, GoogleAuthorizeInput};
use irongate::routes::create_router;
use irongate::store::keys::StoreKey;
use irongate::store::records::{AuthorizeSessionRecord, ProviderStateRecord};
use irongate::store::AuthStore;
use irongate::StorageAdapter;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;
use url::Url;

mod support;
use support::{NoopEmailSender, TestStorage};

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn write_client_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-google-client-config-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&path, contents).expect("write client config");
    path
}

fn runtime_with_google_config(google_enabled: bool) -> Arc<RuntimeAuthConfig> {
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
    let mut env = HashMap::from([
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

    if google_enabled {
        env.insert(
            "AUTH_GOOGLE_CLIENT_ID".to_string(),
            "google-client-id".to_string(),
        );
        env.insert(
            "AUTH_GOOGLE_CLIENT_SECRET".to_string(),
            "google-client-secret".to_string(),
        );
    }

    Arc::new(
        RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
            .expect("runtime config"),
    )
}

fn google_app_state(google_enabled: bool) -> AppState<TestStorage> {
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".to_string());
    AppState {
        storage: Arc::new(TestStorage::new()),
        config: Arc::new(config),
        runtime: runtime_with_google_config(google_enabled),
        providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
        email_sender: Arc::new(NoopEmailSender::default()),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
    }
}

#[tokio::test]
async fn provider_state_store_uses_hmac_key_and_consumes_once() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let raw_state = "raw-google-provider-state";
    let raw_session = "raw-authorize-session";
    let state_digest = lookup_digest(LOOKUP_SECRET, LookupFamily::ProviderState, raw_state);
    let session_digest = lookup_digest(
        LOOKUP_SECRET,
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    let expires_at = Utc::now() + Duration::minutes(10);

    store
        .create_provider_state(
            &state_digest,
            ProviderStateRecord {
                session_lookup_digest: session_digest.clone(),
                provider: "google".to_string(),
                pkce_verifier: "pkce-verifier".to_string(),
                nonce: "provider-nonce".to_string(),
                created_at: Utc::now(),
                expires_at,
            },
        )
        .await
        .expect("create provider state");

    let key = StoreKey::provider_state(&state_digest);
    assert_ne!(key.sk(), raw_state);
    let stored = storage
        .get(&[key.pk(), key.sk()])
        .await
        .expect("get provider state")
        .expect("provider state");
    let record: ProviderStateRecord =
        serde_json::from_value(stored).expect("provider state json");
    assert_eq!(record.session_lookup_digest, session_digest);
    assert_eq!(record.provider, "google");
    assert_eq!(record.pkce_verifier, "pkce-verifier");
    assert_eq!(record.nonce, "provider-nonce");
    assert_eq!(record.expires_at, expires_at);

    let all_state = storage
        .scan(&["provider:state"])
        .await
        .expect("scan provider state");
    let debug = format!("{all_state:?}");
    assert!(!debug.contains(raw_state));
    assert!(!debug.contains(raw_session));

    let consumed = store
        .take_provider_state(&state_digest)
        .await
        .expect("take provider state")
        .expect("provider state exists");
    assert_eq!(consumed.nonce, "provider-nonce");
    assert!(store
        .take_provider_state(&state_digest)
        .await
        .expect("take provider state again")
        .is_none());
}

#[tokio::test]
async fn provider_state_store_rejects_expired_records() {
    let store = AuthStore::new(TestStorage::new());

    store
        .create_provider_state(
            "expired-provider-state-digest",
            ProviderStateRecord {
                session_lookup_digest: "session-digest".to_string(),
                provider: "google".to_string(),
                pkce_verifier: "pkce-verifier".to_string(),
                nonce: "provider-nonce".to_string(),
                created_at: Utc::now() - Duration::minutes(11),
                expires_at: Utc::now() - Duration::seconds(1),
            },
        )
        .await
        .expect("create expired provider state");

    assert!(store
        .take_provider_state("expired-provider-state-digest")
        .await
        .expect("take expired provider state")
        .is_none());
}

#[test]
fn google_authorization_url_contains_oidc_state_nonce_and_pkce_without_secret() {
    let google = GoogleConfig::from_values(Some("google-client-id"), Some("google-secret"))
        .expect("google config")
        .expect("google enabled");

    let url = build_google_authorization_url(GoogleAuthorizeInput {
        config: &google,
        redirect_uri: "https://auth.example.com/google/callback",
        state: "raw-provider-state",
        nonce: "raw-provider-nonce",
        pkce_challenge: "provider-pkce-challenge",
    });

    let parsed = Url::parse(&url).expect("google authorize url");
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("accounts.google.com"));
    assert_eq!(parsed.path(), "/o/oauth2/v2/auth");

    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("google-client-id")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("https://auth.example.com/google/callback")
    );
    assert_eq!(
        query.get("response_type").map(String::as_str),
        Some("code")
    );
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("openid email profile")
    );
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
    assert!(!url.contains("google-secret"));
}

#[tokio::test]
async fn authorize_rejects_google_provider_when_google_is_disabled() {
    let app = create_router(google_app_state(false));
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&scope=openid%20email&provider=google&nonce=client-nonce&code_challenge=challenge&code_challenge_method=S256";

    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn authorize_accepts_google_provider_and_redirects_to_google_start() {
    let state = google_app_state(true);
    let storage = state.storage.clone();
    let app = create_router(state);
    let uri = "/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=abc&scope=openid%20email&provider=google&nonce=client-nonce&code_challenge=challenge&code_challenge_method=S256";

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
    assert!(location.starts_with("/google/authorize?session="));
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
    assert_eq!(sessions[0].1["selected_provider"], "google");
    assert_eq!(sessions[0].1["oidc_nonce"], "client-nonce");
}

#[tokio::test]
async fn google_authorize_route_creates_provider_state_and_redirects_to_google() {
    let state = google_app_state(true);
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
    let store = AuthStore::new(storage.clone());
    let raw_session = "raw-google-authorize-session";
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
                selected_provider: Some("google".to_string()),
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
                .uri(format!("/google/authorize?session={raw_session}"))
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
    let parsed = Url::parse(location).expect("google redirect url");
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("accounts.google.com"));

    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("google-client-id")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("https://auth.example.com/google/callback")
    );
    assert_eq!(
        query.get("response_type").map(String::as_str),
        Some("code")
    );
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("openid email profile")
    );
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    let raw_provider_state = query.get("state").expect("provider state");
    assert!(!raw_provider_state.is_empty());
    assert!(query.get("nonce").is_some());
    assert!(query.get("code_challenge").is_some());
    assert!(!location.contains("google-client-secret"));

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
    assert_eq!(provider_states[0].1["provider"], "google");
}

#[tokio::test]
async fn google_authorize_route_rejects_non_google_authorize_session() {
    let state = google_app_state(true);
    let runtime = state.runtime.clone();
    let storage = state.storage.clone();
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
                .uri(format!("/google/authorize?session={raw_session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
