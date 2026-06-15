use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::clients::{ClientRegistry, GrantType};
use irongate::crypto::signing::LocalEs256Signer;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn write_client_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-client-config-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&path, contents).expect("write client config");
    path
}

fn base_env(client_config_path: &PathBuf) -> HashMap<String, String> {
    let signer = LocalEs256Signer::generate().expect("signer");
    HashMap::from([
        (
            "AUTH_CLIENT_CONFIG_PATH".to_string(),
            client_config_path.display().to_string(),
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
    ])
}

fn public_client_config() -> &'static str {
    r#"
[[clients]]
client_id = "web"
client_type = "public"
redirect_uris = ["https://app.example.com/auth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email"]
pkce_required = true
token_endpoint_auth_method = "none"
"#
}

#[test]
fn runtime_config_loads_client_file_and_required_secrets() {
    let path = write_client_config(public_client_config());
    let env = base_env(&path);

    let runtime = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect("runtime config");

    assert!(runtime.client_registry.get("web").is_some());
    assert_eq!(runtime.lookup_secret.as_bytes().len(), 32);
    assert_eq!(runtime.ttls.access_token_seconds, 3600);
    assert_eq!(runtime.signer.kid(), "test-key");
}

#[test]
fn runtime_config_fails_when_client_file_is_missing() {
    let path = std::env::temp_dir().join("irongate-missing-client-config.toml");
    let env = base_env(&path);

    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing file should fail");

    assert!(err.to_string().contains("client config"));
}

#[test]
fn runtime_config_fails_when_hmac_secret_is_missing() {
    let path = write_client_config(public_client_config());
    let mut env = base_env(&path);
    env.remove("AUTH_HMAC_LOOKUP_SECRET");

    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing lookup secret should fail");

    assert!(err.to_string().contains("AUTH_HMAC_LOOKUP_SECRET"));
}

#[test]
fn runtime_config_fails_when_confidential_client_secret_is_missing() {
    let config = r#"
[[clients]]
client_id = "backend"
client_type = "confidential"
redirect_uris = ["https://api.example.com/auth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email"]
pkce_required = false
token_endpoint_auth_method = "client_secret_basic"
client_secret_ref = "AUTH_CLIENT_BACKEND_SECRET"
"#;
    let path = write_client_config(config);
    let env = base_env(&path);

    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing client secret should fail");

    assert!(err.to_string().contains("AUTH_CLIENT_BACKEND_SECRET"));
}

#[test]
fn client_registry_validates_exact_redirect_pkce_secret_and_grants() {
    let path = write_client_config(public_client_config());
    let env = base_env(&path);
    let runtime = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect("runtime config");
    let registry: &ClientRegistry = &runtime.client_registry;

    let client = registry
        .validate_authorize_request(
            "web",
            "https://app.example.com/auth/callback",
            "code",
            Some("challenge"),
        )
        .expect("valid authorize request");
    assert_eq!(client.client_id, "web");

    assert!(registry
        .validate_authorize_request(
            "web",
            "https://app.example.com/other",
            "code",
            Some("challenge"),
        )
        .is_err());
    assert!(registry
        .validate_authorize_request(
            "web",
            "https://app.example.com/auth/callback",
            "code",
            None,
        )
        .is_err());
    assert!(registry
        .validate_token_request("web", GrantType::RefreshToken, None)
        .is_ok());
    assert!(registry.validate_token_grant("web", "client_credentials").is_err());
}
