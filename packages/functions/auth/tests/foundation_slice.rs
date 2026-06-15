use chrono::{Duration, Utc};
use irongate::config::account_lifecycle::AccountLifecycleConfig;
use irongate::config::audit::AuditLogMode;
use irongate::config::client_file::ClientFile;
use irongate::config::signing::SigningConfig;
use irongate::config::ttls::TtlConfig;
use irongate::core::clients::GrantType;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::password::verify_password;
use irongate::crypto::signing::LocalEs256Signer;
use irongate::oauth::well_known::build_authorization_server_metadata;
use irongate::store::keys::StoreKey;
use irongate::store::{AuthStore, DeletedIdentityReusePolicy, IdentityProvider};
use std::str::FromStr;

mod support;
use support::TestStorage;

#[test]
fn discovery_metadata_advertises_only_foundation_flows() {
    let metadata = build_authorization_server_metadata("https://auth.example.com");

    assert_eq!(metadata.issuer, "https://auth.example.com");
    assert_eq!(
        metadata.grant_types_supported,
        vec![
            "authorization_code".to_string(),
            "refresh_token".to_string()
        ]
    );
    assert_eq!(metadata.response_types_supported, vec!["code".to_string()]);
    assert_eq!(
        metadata.id_token_signing_alg_values_supported,
        vec!["ES256".to_string()]
    );
    let metadata_json = serde_json::to_value(&metadata).expect("metadata json");
    assert!(metadata_json.get("revocation_endpoint").is_none());
    assert!(!metadata
        .grant_types_supported
        .contains(&"client_credentials".to_string()));
}

#[test]
fn runtime_config_primitives_validate_security_bounds() {
    TtlConfig::default()
        .validate()
        .expect("default ttls are valid");

    let invalid_ttls = TtlConfig {
        auth_code_seconds: 601,
        ..TtlConfig::default()
    };
    assert!(invalid_ttls.validate().is_err());

    let lifecycle =
        AccountLifecycleConfig::from_values("after_retention", 30).expect("lifecycle config");
    assert_eq!(
        lifecycle.deleted_identity_reuse,
        DeletedIdentityReusePolicy::AfterRetention
    );
    assert!(AccountLifecycleConfig::from_values("after_retention", 0).is_err());
    assert_eq!(
        AuditLogMode::from_str("cloudwatch").expect("audit mode"),
        AuditLogMode::CloudWatch
    );
    assert!(AuditLogMode::from_str("s3").is_err());

    let signing = SigningConfig::from_values(
        "local-es256",
        Some("local-key-1"),
        Some("AUTH_SIGNING_PRIVATE_KEY"),
        None,
    )
    .expect("signing config");
    assert_eq!(signing.key_id, "local-key-1");
    assert!(SigningConfig::from_values("local-es256", Some("key"), None, None).is_err());
}

#[test]
fn client_config_rejects_runtime_and_invalid_secret_shapes() {
    let client_credentials = r#"
[[clients]]
client_id = "worker"
client_type = "confidential"
redirect_uris = ["https://app.example.com/callback"]
allowed_grant_types = ["client_credentials"]
allowed_scopes = ["openid"]
pkce_required = true
token_endpoint_auth_method = "client_secret_basic"
client_secret_ref = "CLIENT_SECRET"
"#;

    let public_with_secret = r#"
[[clients]]
client_id = "web"
client_type = "public"
redirect_uris = ["https://app.example.com/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile"]
pkce_required = true
token_endpoint_auth_method = "none"
client_secret_ref = "CLIENT_SECRET"
"#;

    assert!(ClientFile::from_toml_str(client_credentials).is_err());
    assert!(ClientFile::from_toml_str(public_with_secret).is_err());
}

#[test]
fn client_config_accepts_public_code_refresh_client() {
    let config = r#"
[[clients]]
client_id = "web"
client_type = "public"
redirect_uris = ["https://app.example.com/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email"]
pkce_required = true
token_endpoint_auth_method = "none"
"#;

    let clients = ClientFile::from_toml_str(config).expect("valid client config");
    let client = clients.client("web").expect("client exists");

    assert_eq!(client.client_id, "web");
    assert!(client
        .allowed_grant_types
        .contains(&GrantType::AuthorizationCode));
    assert!(client
        .allowed_grant_types
        .contains(&GrantType::RefreshToken));
    assert!(client.client_secret_hash.is_none());
}

#[test]
fn confidential_client_secret_is_resolved_to_hash() {
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

    let clients = ClientFile::from_toml_str_with_secret_resolver(config, |name| {
        (name == "AUTH_CLIENT_BACKEND_SECRET").then(|| "super-secret-client-value".to_string())
    })
    .expect("valid confidential client config");
    let client = clients.client("backend").expect("client exists");
    let hash = client.client_secret_hash.as_deref().expect("secret hash");

    assert_ne!(hash, "super-secret-client-value");
    assert!(verify_password("super-secret-client-value", hash));
}

#[test]
fn hmac_key_helpers_never_store_raw_bearer_values() {
    let secret = b"template-local-secret-with-enough-bytes";
    let raw_authorization_code = "ig_code_raw_secret_value";
    let raw_refresh_token = "ig_refresh_raw_secret_value";

    let code_digest = lookup_digest(
        secret,
        LookupFamily::AuthorizationCode,
        raw_authorization_code,
    );
    let refresh_digest = lookup_digest(secret, LookupFamily::RefreshToken, raw_refresh_token);

    assert_ne!(code_digest, refresh_digest);
    assert_eq!(
        code_digest,
        lookup_digest(
            secret,
            LookupFamily::AuthorizationCode,
            raw_authorization_code
        )
    );

    let code_key = StoreKey::authorization_code(&code_digest);
    let refresh_key = StoreKey::refresh_token(&refresh_digest);

    for key in [code_key, refresh_key] {
        assert!(!key.pk().contains(raw_authorization_code));
        assert!(!key.sk().contains(raw_authorization_code));
        assert!(!key.pk().contains(raw_refresh_token));
        assert!(!key.sk().contains(raw_refresh_token));
    }
}

#[test]
fn local_es256_signer_jwks_contains_public_material_only() {
    let signer = LocalEs256Signer::generate().expect("signer");
    let jwks_json = serde_json::to_value(signer.jwks()).expect("jwks json");
    let first_key = jwks_json["keys"][0].as_object().expect("jwk object");

    assert_eq!(
        first_key.get("kid").and_then(|v| v.as_str()),
        Some(signer.kid())
    );
    assert_eq!(first_key.get("alg").and_then(|v| v.as_str()), Some("ES256"));
    assert!(first_key.contains_key("x"));
    assert!(first_key.contains_key("y"));
    assert!(!jwks_json.to_string().contains("PRIVATE KEY"));
    assert!(!first_key.contains_key("d"));
}

#[tokio::test]
async fn deleted_identity_reuse_allocates_new_subject() {
    let store = AuthStore::new(TestStorage::new());
    let email_digest = lookup_digest(
        b"template-local-secret-with-enough-bytes",
        LookupFamily::Email,
        "user@example.com",
    );

    let original = store
        .create_account_with_identity(
            IdentityProvider::Password,
            &email_digest,
            serde_json::json!({ "email": "user@example.com" }),
        )
        .await
        .expect("initial identity");

    store
        .delete_identity(
            IdentityProvider::Password,
            &email_digest,
            Utc::now() - Duration::seconds(1),
        )
        .await
        .expect("delete identity");

    let replacement = store
        .reuse_deleted_identity(
            IdentityProvider::Password,
            &email_digest,
            DeletedIdentityReusePolicy::AfterRetention,
            serde_json::json!({ "email": "user@example.com" }),
        )
        .await
        .expect("reused identity");

    assert_ne!(original.as_str(), replacement.as_str());
}
