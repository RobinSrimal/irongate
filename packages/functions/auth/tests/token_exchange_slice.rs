use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config, ProviderConfig};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::oauth::well_known::build_authorization_server_metadata;
use irongate::routes::create_router;
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
        ("AUTH_SIGNING_KEY_ID".to_string(), "slice-05-key".to_string()),
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
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".to_string());
    AppState {
        storage: Arc::new(TestStorage::new()),
        config: Arc::new(config),
        runtime: runtime_with_public_client(),
        providers: Arc::new(HashMap::<String, ProviderConfig>::new()),
        email_sender: Arc::new(NoopEmailSender),
    }
}

#[test]
fn slice_05_metadata_advertises_only_authorization_code_behavior() {
    let metadata = build_authorization_server_metadata("https://auth.example.com");
    let metadata_json = serde_json::to_value(&metadata).expect("metadata json");

    assert_eq!(
        metadata.grant_types_supported,
        vec!["authorization_code".to_string()]
    );
    assert!(!metadata
        .grant_types_supported
        .contains(&"refresh_token".to_string()));
    assert!(!metadata
        .scopes_supported
        .contains(&"offline_access".to_string()));
    assert!(metadata_json.get("revocation_endpoint").is_none());
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
