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
    assert_eq!(runtime.email.from, "Irongate <auth@example.com>");
    assert_eq!(
        runtime.email.verify_url_base.as_str(),
        "https://app.example.com/auth/verify-email"
    );
    assert_eq!(
        runtime.email.reset_url_base.as_str(),
        "https://app.example.com/auth/reset-password"
    );
    assert_eq!(runtime.access_token_audience, "https://api.example.com");
    assert!(runtime.google.is_none());
    assert!(runtime.apple.is_none());
}

#[test]
fn runtime_config_loads_google_when_client_id_and_secret_are_present() {
    let path = write_client_config(public_client_config());
    let mut env = base_env(&path);
    env.insert(
        "AUTH_GOOGLE_CLIENT_ID".to_string(),
        "google-client.apps.googleusercontent.com".to_string(),
    );
    env.insert(
        "AUTH_GOOGLE_CLIENT_SECRET".to_string(),
        "google-secret-value".to_string(),
    );

    let runtime = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect("runtime config");
    let google = runtime.google.as_ref().expect("google config");

    assert_eq!(google.client_id, "google-client.apps.googleusercontent.com");
    assert_eq!(
        google.authorization_url.as_str(),
        "https://accounts.google.com/o/oauth2/v2/auth"
    );
    assert_eq!(google.scopes, vec!["openid", "email", "profile"]);
    let debug = format!("{runtime:?}");
    assert!(debug.contains("google"));
    assert!(!debug.contains("google-secret-value"));
}

#[test]
fn runtime_config_rejects_half_configured_google() {
    let path = write_client_config(public_client_config());
    let mut env = base_env(&path);
    env.insert(
        "AUTH_GOOGLE_CLIENT_ID".to_string(),
        "google-client.apps.googleusercontent.com".to_string(),
    );

    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing google secret should fail");
    assert!(err.to_string().contains("Google"));

    let mut env = base_env(&path);
    env.insert(
        "AUTH_GOOGLE_CLIENT_SECRET".to_string(),
        "google-secret-value".to_string(),
    );
    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing google client id should fail");
    assert!(err.to_string().contains("Google"));
}

#[test]
fn runtime_config_loads_apple_when_required_values_and_private_key_secret_are_present() {
    let path = write_client_config(public_client_config());
    let mut env = base_env(&path);
    env.insert(
        "AUTH_APPLE_CLIENT_ID".to_string(),
        "com.example.web".to_string(),
    );
    env.insert("AUTH_APPLE_TEAM_ID".to_string(), "TEAMID1234".to_string());
    env.insert("AUTH_APPLE_KEY_ID".to_string(), "KEYID12345".to_string());
    env.insert(
        "AUTH_APPLE_PRIVATE_KEY_SECRET".to_string(),
        "AUTH_SIGNING_PRIVATE_KEY".to_string(),
    );

    let runtime = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect("runtime config");
    let apple = runtime.apple.as_ref().expect("apple config");

    assert_eq!(apple.client_id, "com.example.web");
    assert_eq!(apple.team_id, "TEAMID1234");
    assert_eq!(apple.key_id, "KEYID12345");
    assert_eq!(
        apple.authorization_url.as_str(),
        "https://appleid.apple.com/auth/authorize"
    );
    assert_eq!(apple.scopes, vec!["name", "email"]);
    assert_eq!(apple.client_secret_ttl_seconds, 86_400);

    let debug = format!("{runtime:?}");
    assert!(debug.contains("apple"));
    assert!(!debug.contains("BEGIN PRIVATE KEY"));
}

#[test]
fn runtime_config_rejects_incomplete_or_invalid_apple_config() {
    let path = write_client_config(public_client_config());
    let mut env = base_env(&path);
    env.insert(
        "AUTH_APPLE_CLIENT_ID".to_string(),
        "com.example.web".to_string(),
    );
    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("half configured apple should fail");
    assert!(err.to_string().contains("Apple"));

    let mut env = base_env(&path);
    env.insert(
        "AUTH_APPLE_CLIENT_ID".to_string(),
        "com.example.web".to_string(),
    );
    env.insert("AUTH_APPLE_TEAM_ID".to_string(), "TEAMID1234".to_string());
    env.insert("AUTH_APPLE_KEY_ID".to_string(), "KEYID12345".to_string());
    env.insert(
        "AUTH_APPLE_PRIVATE_KEY_SECRET".to_string(),
        "MISSING_APPLE_PRIVATE_KEY".to_string(),
    );
    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing apple private key secret should fail");
    assert!(err.to_string().contains("MISSING_APPLE_PRIVATE_KEY"));

    let mut env = base_env(&path);
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
        "not a private key".to_string(),
    );
    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("invalid apple private key should fail");
    assert!(err.to_string().contains("Apple"));

    let mut env = base_env(&path);
    env.insert(
        "AUTH_APPLE_CLIENT_ID".to_string(),
        "com.example.web".to_string(),
    );
    env.insert("AUTH_APPLE_TEAM_ID".to_string(), "TEAMID1234".to_string());
    env.insert("AUTH_APPLE_KEY_ID".to_string(), "KEYID12345".to_string());
    env.insert(
        "AUTH_APPLE_PRIVATE_KEY_SECRET".to_string(),
        "AUTH_SIGNING_PRIVATE_KEY".to_string(),
    );
    env.insert(
        "AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS".to_string(),
        "0".to_string(),
    );
    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("invalid apple ttl should fail");
    assert!(err
        .to_string()
        .contains("AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS"));
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
fn runtime_config_fails_when_resend_api_key_is_missing() {
    let path = write_client_config(public_client_config());
    let mut env = base_env(&path);
    env.remove("RESEND_API_KEY");

    let err = RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
        .expect_err("missing resend key should fail");

    assert!(err.to_string().contains("RESEND_API_KEY"));
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
        .validate_authorize_request("web", "https://app.example.com/auth/callback", "code", None,)
        .is_err());
    assert!(registry
        .validate_token_request("web", GrantType::RefreshToken, None)
        .is_ok());
    assert!(registry
        .validate_token_grant("web", "client_credentials")
        .is_err());
}
