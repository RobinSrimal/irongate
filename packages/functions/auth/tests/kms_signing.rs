use async_trait::async_trait;
use chrono::Utc;
use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::tokens::AccessTokenClaims;
use irongate::crypto::signing::{
    der_signature_to_jose, KmsEs256Signer, KmsPublicKey, KmsSigningOperations, SigningMode,
};
use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;
use rand::rngs::OsRng;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FakeKms {
    signing_key: SigningKey,
    last_digest: Arc<Mutex<Option<Vec<u8>>>>,
}

impl FakeKms {
    fn new(signing_key: SigningKey) -> Self {
        Self {
            signing_key,
            last_digest: Arc::new(Mutex::new(None)),
        }
    }

    fn last_digest(&self) -> Option<Vec<u8>> {
        self.last_digest.lock().expect("digest lock").clone()
    }
}

#[async_trait]
impl KmsSigningOperations for FakeKms {
    async fn get_public_key(&self, _key_id: &str) -> Result<KmsPublicKey, String> {
        Ok(KmsPublicKey {
            der: self
                .signing_key
                .verifying_key()
                .to_public_key_der()
                .expect("public key der")
                .as_bytes()
                .to_vec(),
            key_spec: "ECC_NIST_P256".to_string(),
            key_usage: "SIGN_VERIFY".to_string(),
        })
    }

    async fn sign_digest(&self, _key_id: &str, digest: &[u8]) -> Result<Vec<u8>, String> {
        *self.last_digest.lock().expect("digest lock") = Some(digest.to_vec());
        let signature: Signature = self
            .signing_key
            .sign_prehash(digest)
            .map_err(|err| err.to_string())?;
        Ok(signature.to_der().as_bytes().to_vec())
    }
}

fn access_claims() -> AccessTokenClaims {
    let now = Utc::now().timestamp();
    AccessTokenClaims {
        mode: "access".to_string(),
        iss: "https://auth.example.com".to_string(),
        sub: "user_123".to_string(),
        aud: "https://api.example.com".to_string(),
        iat: now,
        exp: now + 3600,
        scope: "openid email".to_string(),
        subject_type: "user".to_string(),
        properties: json!({
            "email": "user@example.com",
            "email_verified": true
        }),
    }
}

fn write_client_config() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-kms-client-config-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(
        &path,
        r#"
[[clients]]
client_id = "web"
client_type = "public"
redirect_uris = ["https://app.example.com/auth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email"]
pkce_required = true
token_endpoint_auth_method = "none"
"#,
    )
    .expect("write client config");
    path
}

#[tokio::test]
async fn kms_signer_signs_digest_and_publishes_public_jwks() {
    let signing_key = SigningKey::random(&mut OsRng);
    let fake_kms = FakeKms::new(signing_key);
    let signer = KmsEs256Signer::from_operations(
        "kms-kid-1".to_string(),
        "alias/test/auth-signing".to_string(),
        Arc::new(fake_kms.clone()),
    )
    .await
    .expect("kms signer");

    let token = signer
        .sign_access_token(&access_claims())
        .await
        .expect("signed access token");
    let signing_input = token
        .rsplit_once('.')
        .map(|(input, _signature)| input)
        .expect("compact jwt input");
    let expected_digest = Sha256::digest(signing_input.as_bytes()).to_vec();
    assert_eq!(fake_kms.last_digest(), Some(expected_digest));

    let header = jsonwebtoken::decode_header(&token).expect("token header");
    assert_eq!(header.alg, jsonwebtoken::Algorithm::ES256);
    assert_eq!(header.kid.as_deref(), Some("kms-kid-1"));

    let claims = signer
        .verify_access_token(
            &token,
            "https://auth.example.com",
            "https://api.example.com",
        )
        .expect("verified access token");
    assert_eq!(claims.sub, "user_123");
    assert_eq!(claims.mode, "access");

    let jwks = serde_json::to_value(signer.jwks()).expect("jwks json");
    assert_eq!(jwks["keys"][0]["kid"], "kms-kid-1");
    assert_eq!(jwks["keys"][0]["alg"], "ES256");
    assert_eq!(jwks["keys"][0]["crv"], "P-256");
    assert!(jwks["keys"][0].get("x").is_some());
    assert!(jwks["keys"][0].get("y").is_some());
    assert!(jwks["keys"][0].get("d").is_none());
}

#[test]
fn der_signature_conversion_rejects_invalid_der() {
    assert!(der_signature_to_jose(b"not a der signature").is_err());
}

#[tokio::test]
async fn runtime_config_accepts_kms_mode_without_local_private_key() {
    let signing_key = SigningKey::random(&mut OsRng);
    let fake_kms = Arc::new(FakeKms::new(signing_key));
    let client_config_path = write_client_config();
    let env = HashMap::from([
        (
            "AUTH_CLIENT_CONFIG_PATH".to_string(),
            client_config_path.display().to_string(),
        ),
        (
            "AUTH_HMAC_LOOKUP_SECRET".to_string(),
            "0123456789abcdef0123456789abcdef".to_string(),
        ),
        ("AUTH_SIGNING_MODE".to_string(), "kms-es256".to_string()),
        (
            "AUTH_SIGNING_KEY_ID".to_string(),
            "kms-runtime-kid".to_string(),
        ),
        (
            "AUTH_SIGNING_KMS_KEY_ID".to_string(),
            "alias/test/auth-signing".to_string(),
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

    let runtime = RuntimeAuthConfig::from_env_map_with_kms_operations(
        &env,
        |name| env.get(name).cloned(),
        fake_kms,
    )
    .await
    .expect("kms runtime config");

    assert_eq!(runtime.signing.mode, SigningMode::KmsEs256);
    assert_eq!(runtime.signer.kid(), "kms-runtime-kid");
    assert_eq!(
        runtime
            .client_registry
            .get("web")
            .expect("client")
            .client_id,
        "web"
    );
}
