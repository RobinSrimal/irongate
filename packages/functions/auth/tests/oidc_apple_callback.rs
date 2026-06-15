use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{header::LOCATION, Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::config::apple::APPLE_ISSUER;
use irongate::config::environment::RuntimeAuthConfig;
use irongate::config::{AppState, Config};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::oauth::pkce::generate_challenge;
use irongate::providers::apple::{
    apple_identity_digest, validate_apple_id_token, AppleCodeExchangeInput, AppleIdTokenValidation,
    AppleJwk, AppleJwks, AppleOidcClient, AppleOidcError, AppleTokenResponse,
};
use irongate::routes::create_router;
use irongate::storage::StorageAdapter;
use irongate::store::records::{AuthorizeSessionRecord, ProviderStateRecord};
use irongate::store::{AuthStore, DeletedIdentityReusePolicy, IdentityProvider};
use jsonwebtoken::{decode_header, encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;
use url::Url;

mod support;
use support::{NoopEmailSender, TestStorage};

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";
const APPLE_CLIENT_ID: &str = "com.example.web";
const PROVIDER_NONCE: &str = "provider-nonce";
const TEST_KEY_ID: &str = "apple-test-key";
const TEST_RSA_N: &str = "1okldhpIZquS0duQN26-ooaOE2ywCuYI9vMmS5iq6tIHqn62ApyNn4Ax6CAtjkdnAr9XexbCm6TdRKCh75p3KZMiiVH0Ws7iRQhncn-yHDAFLr8b5is7pKEZ53JqVtAAdk2LCBv38Ms58tYeZelU6Q8R6kaKuxsut5RanmS-YbsG59ThzNAZQLHjG1od8T_dCRpFQfOrP1UJa5sWRVhiBng09eH32A5E-onrbY2Ac7pFOpHpsir_rQutcjzjOwhO4jG1r0FPavXLi0yIisXH_cY5HgGkBUEccpcqESruOjwCBfxcPOMXdZtO2z73w9LqlBrjpohjGGe6QIUAsVoZbQ";
const TEST_RSA_E: &str = "AQAB";
const TEST_RSA_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDWiSV2Gkhmq5LR
25A3br6iho4TbLAK5gj28yZLmKrq0geqfrYCnI2fgDHoIC2OR2cCv1d7FsKbpN1E
oKHvmncpkyKJUfRazuJFCGdyf7IcMAUuvxvmKzukoRnncmpW0AB2TYsIG/fwyzny
1h5l6VTpDxHqRoq7Gy63lFqeZL5huwbn1OHM0BlAseMbWh3xP90JGkVB86s/VQlr
mxZFWGIGeDT14ffYDkT6iettjYBzukU6kemyKv+tC61yPOM7CE7iMbWvQU9q9cuL
TIiKxcf9xjkeAaQFQRxylyoRKu46PAIF/Fw84xd1m07bPvfD0uqUGuOmiGMYZ7pA
hQCxWhltAgMBAAECggEACPlU4v3gkf0Z3tkRTToUMB85xE/ooXlpFuvUTYkdCSmp
Zd/bIKdkzdm3w9J2+rR0d3lX2g+HnMXjEugaynBnKYrgVjx+/SIZ9bJIIe7RK4of
WrWCyoaYU1+ryVXXYzrN1bM9c6SqFM8VOoSWDNJ+/QyDDQ4zWKDYZrR4HiXvq6o/
/Qf9mPBLOh12p2IZ85L9f9fLTL4uYUUHSKKAqfWN/DLb7jinnUdok55I47qYuHtH
YFpQK0/3ZnCcbRIzooVOO3bSKbHXACSdZMrTKfk8ELFi1EjaMin6bgsS3SDlSikR
kT2t0rIvfUibh9WRZNtExLEtPPdk7izTDSlpVPHCjwKBgQD3R4kjLxYWIOzOfrGl
H1W1kKHTtKLpsgISGGdaBSSd4fnIpWDIkWs8PlBXadVNHVLBpTuF1s4VxnxJGBVL
XzHbgOohiv0e7M5DHm9TaSPBKANBc0qBlUKdYuE2GfligRuWrSStzfOTL/uh5hh0
cBm8LoZigW9ndw8v8ZIN5LQmywKBgQDeGf8/F5zEi4bbXzVzE3mmyyeeCl+BHJ0g
1Dspndm3/qA55pWcBZU2GKaqK8mXEZytjVM6geo9Z4l5hIH0Fcr03KZJ49zdASe/
U+e3nfjOq/TTrsqt7LjwEEVOGRKYy/jgS9rTEnBKYI+a51ysvT3grfphvo+K7k0R
vHsSH0oBpwKBgQCW5/4mDadB6+f4gNLyvTO2MUTBCRze13ZyCpiQFFFrVKv2Kg7t
d+lkg3bOUdUNUZbefHLd0+BC47WXee4M6FRp67t2qvacN9IMnfc8hQ5/42ZRPAW9
HRThLaXZOXK7DaWDh7i5pNU//ulmvSAxdvQNpqr2VJ1jHAKVtKv4dJkIjwKBgHDj
p+BKwS0JeldAkmtWZ8wGkLF8tkRq5dbM6PFjQUmLS6eCc2LlV40yhGwUa5e0pP11
yur/I69oU/EHEAKfnRROntsJzbYroydVn36t9cwejQeXXX9/xhSHQKLMja5KZsqi
46vLQHYdlIB4vpsyaSQtagmKkW1daKDuO2PfsX8bAoGBAO2lDrTjVUTi0OBSAfDx
zHIJszPyHY/nW4+rrVoE2GmDqFulXZ+gPq6b0G+GHJwzAt/RLMNpy//6D3rG6TzA
mn25y2Yr9HtgOb4aegL+FgOJ7CwINu9lgtbLAKOvYhj2QlVEca927VyUNRHkmeFY
yldT9HITVXtce9FVqgF83Lkz
-----END PRIVATE KEY-----"#;

#[derive(Clone)]
struct FakeAppleOidcClient {
    id_token: Arc<String>,
}

#[async_trait]
impl AppleOidcClient for FakeAppleOidcClient {
    async fn exchange_code(
        &self,
        _config: &irongate::config::apple::AppleConfig,
        input: AppleCodeExchangeInput<'_>,
    ) -> Result<AppleTokenResponse, AppleOidcError> {
        assert_eq!(input.code, "apple-code");
        assert_eq!(
            input.redirect_uri,
            "https://auth.example.com/apple/callback"
        );
        assert_eq!(input.code_verifier, "provider-pkce-verifier");
        assert!(!input.client_secret.contains("BEGIN PRIVATE KEY"));
        let header = decode_header(input.client_secret).expect("apple client secret header");
        assert_eq!(header.alg, Algorithm::ES256);
        assert_eq!(header.kid.as_deref(), Some("KEYID12345"));
        Ok(AppleTokenResponse {
            access_token: Some("apple-access-token".to_string()),
            token_type: Some("Bearer".to_string()),
            expires_in: Some(3600),
            refresh_token: Some("apple-refresh-token".to_string()),
            id_token: self.id_token.as_ref().clone(),
        })
    }

    async fn fetch_jwks(
        &self,
        _config: &irongate::config::apple::AppleConfig,
    ) -> Result<AppleJwks, AppleOidcError> {
        Ok(jwks())
    }
}

#[derive(Debug, Serialize)]
struct TestAppleClaims<'a> {
    iss: &'a str,
    sub: &'a str,
    aud: &'a str,
    exp: i64,
    iat: i64,
    nonce: &'a str,
    email: Option<&'a str>,
    email_verified: Option<&'a str>,
    is_private_email: Option<&'a str>,
}

#[test]
fn apple_identity_digest_uses_issuer_and_subject_not_email() {
    let digest = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "apple-sub-a");
    let same = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "apple-sub-a");
    let different_sub = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "apple-sub-b");
    let different_issuer =
        apple_identity_digest(LOOKUP_SECRET, "https://other.example", "apple-sub-a");

    assert_eq!(digest, same);
    assert_ne!(digest, different_sub);
    assert_ne!(digest, different_issuer);
    assert_eq!(
        digest,
        lookup_digest(
            LOOKUP_SECRET,
            LookupFamily::AppleIdentity,
            "https://appleid.apple.com\napple-sub-a",
        )
    );
}

#[test]
fn valid_apple_id_token_validates_signature_nonce_and_claims() {
    let now = Utc::now();
    let token = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });

    let claims =
        validate_apple_id_token(&token, &jwks(), validation(now)).expect("valid apple token");

    assert_eq!(claims.iss, APPLE_ISSUER);
    assert_eq!(claims.sub, "apple-subject");
    assert_eq!(claims.email.as_deref(), Some("user@example.com"));
    assert_eq!(claims.email_verified, Some(true));
    assert_eq!(claims.is_private_email, Some(false));
    assert_eq!(claims.nonce.as_deref(), Some(PROVIDER_NONCE));
}

#[test]
fn apple_id_token_validation_rejects_wrong_security_claims() {
    let now = Utc::now();

    let wrong_issuer = sign_apple_id_token(TestAppleClaims {
        iss: "https://evil.example",
        sub: "apple-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    assert!(validate_apple_id_token(&wrong_issuer, &jwks(), validation(now)).is_err());

    let wrong_audience = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-subject",
        aud: "other-client",
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    assert!(validate_apple_id_token(&wrong_audience, &jwks(), validation(now)).is_err());

    let wrong_nonce = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: "wrong-nonce",
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    assert!(validate_apple_id_token(&wrong_nonce, &jwks(), validation(now)).is_err());

    let expired = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now - Duration::minutes(1)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    assert!(validate_apple_id_token(&expired, &jwks(), validation(now)).is_err());

    let future_iat = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: (now + Duration::minutes(10)).timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    assert!(validate_apple_id_token(&future_iat, &jwks(), validation(now)).is_err());

    let empty_subject = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("user@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    assert!(validate_apple_id_token(&empty_subject, &jwks(), validation(now)).is_err());
}

#[tokio::test]
async fn apple_identity_resolution_creates_and_reuses_active_subject() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let identity_digest = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "apple-subject");

    let subject = store
        .resolve_or_create_apple_identity(
            &identity_digest,
            apple_identity_properties(Some("user@example.com")),
            DeletedIdentityReusePolicy::AfterRetention,
        )
        .await
        .expect("create apple identity");
    let identity = store
        .get_identity(IdentityProvider::Apple, &identity_digest)
        .await
        .expect("get identity")
        .expect("identity exists");
    assert_eq!(identity.subject.as_deref(), Some(subject.as_str()));
    assert_eq!(identity.provider, "apple");
    let properties = identity.properties.as_ref().expect("identity properties");
    assert_eq!(properties["email"], "user@example.com");
    assert!(identity.last_seen_at >= identity.created_at);

    let first_seen = identity.last_seen_at;
    let returned_subject = store
        .resolve_or_create_apple_identity(
            &identity_digest,
            apple_identity_properties(Some("user@example.com")),
            DeletedIdentityReusePolicy::AfterRetention,
        )
        .await
        .expect("reuse apple identity");
    let updated = store
        .get_identity(IdentityProvider::Apple, &identity_digest)
        .await
        .expect("get updated identity")
        .expect("updated identity exists");

    assert_eq!(returned_subject.as_str(), subject.as_str());
    assert!(updated.last_seen_at >= first_seen);

    let identities = storage
        .query_prefix(&["identity:apple"])
        .await
        .expect("query_prefix apple identities");
    assert_eq!(identities.len(), 1);
    let debug = format!("{identities:?}");
    assert!(!debug.contains("apple-subject"));
}

#[tokio::test]
async fn apple_identity_resolution_does_not_auto_link_by_email() {
    let store = AuthStore::new(TestStorage::new());
    let email = "same@example.com";
    let password_digest = lookup_digest(LOOKUP_SECRET, LookupFamily::PasswordIdentity, email);
    let password_subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            &password_digest,
            json!({
                "provider": "password",
                "email": email,
                "email_verified": true
            }),
        )
        .await
        .expect("create password identity");
    let google_digest = lookup_digest(
        LOOKUP_SECRET,
        LookupFamily::GoogleIdentity,
        "https://accounts.google.com\ngoogle-subject",
    );
    let google_subject = store
        .create_account_with_identity(
            IdentityProvider::Google,
            &google_digest,
            json!({
                "provider": "google",
                "issuer": "https://accounts.google.com",
                "email": email,
                "email_verified": true
            }),
        )
        .await
        .expect("create google identity");

    let apple_digest = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "apple-subject");
    let apple_subject = store
        .resolve_or_create_apple_identity(
            &apple_digest,
            apple_identity_properties(Some(email)),
            DeletedIdentityReusePolicy::AfterRetention,
        )
        .await
        .expect("create apple identity");

    assert_ne!(apple_subject.as_str(), password_subject.as_str());
    assert_ne!(apple_subject.as_str(), google_subject.as_str());
}

#[tokio::test]
async fn apple_identity_resolution_applies_deleted_identity_reuse_policy() {
    let store = AuthStore::new(TestStorage::new());
    let identity_digest = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "deleted-subject");
    let subject = store
        .resolve_or_create_apple_identity(
            &identity_digest,
            apple_identity_properties(Some("deleted@example.com")),
            DeletedIdentityReusePolicy::AfterRetention,
        )
        .await
        .expect("create apple identity");

    store
        .delete_identity(
            IdentityProvider::Apple,
            &identity_digest,
            Utc::now() + Duration::days(1),
        )
        .await
        .expect("delete identity");

    assert!(store
        .resolve_or_create_apple_identity(
            &identity_digest,
            apple_identity_properties(Some("deleted@example.com")),
            DeletedIdentityReusePolicy::Never,
        )
        .await
        .is_err());
    assert!(store
        .resolve_or_create_apple_identity(
            &identity_digest,
            apple_identity_properties(Some("deleted@example.com")),
            DeletedIdentityReusePolicy::AfterRetention,
        )
        .await
        .is_err());

    let replacement_subject = store
        .resolve_or_create_apple_identity(
            &identity_digest,
            apple_identity_properties(Some("deleted@example.com")),
            DeletedIdentityReusePolicy::Immediate,
        )
        .await
        .expect("reuse deleted identity");

    assert_ne!(replacement_subject.as_str(), subject.as_str());
}

#[tokio::test]
async fn apple_callback_creates_internal_code_and_redirects_to_client() {
    let now = Utc::now();
    let id_token = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-callback-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("apple@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    let (state, storage) = apple_app_state_with_storage(id_token);
    let runtime = state.runtime.clone();
    seed_apple_callback_state(&storage, &runtime).await;

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/apple/callback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("code=apple-code&state=raw-provider-state"))
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
    let parsed = Url::parse(location).expect("client redirect");
    assert_eq!(
        parsed.as_str().split('?').next().unwrap(),
        "https://app.example.com/auth/callback"
    );
    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    let raw_internal_code = query.get("code").expect("internal code");
    assert_eq!(query.get("state").map(String::as_str), Some("client-state"));

    let provider_states = storage
        .query_prefix(&["provider:state"])
        .await
        .expect("query_prefix provider state");
    let sessions = storage
        .query_prefix(&["oauth:session"])
        .await
        .expect("query_prefix authorize sessions");
    let codes = storage
        .query_prefix(&["oauth:code"])
        .await
        .expect("query_prefix auth codes");
    assert!(provider_states.is_empty());
    assert!(sessions.is_empty());
    assert_eq!(codes.len(), 1);
    assert!(!codes[0]
        .0
        .iter()
        .any(|part| part.contains(raw_internal_code)));
    assert_eq!(codes[0].1["client_id"], "web");
    assert_eq!(codes[0].1["scope"], "openid email");
    assert_eq!(codes[0].1["oidc_nonce"], "client-nonce");
    assert_eq!(codes[0].1["properties"]["provider"], "apple");
    assert_eq!(codes[0].1["properties"]["email"], "apple@example.com");
    assert_eq!(codes[0].1["properties"]["email_verified"], true);

    let storage_debug = format!("{codes:?}");
    assert!(!storage_debug.contains("raw-provider-state"));
    assert!(!storage_debug.contains("apple-code"));
    assert!(!storage_debug.contains("apple-access-token"));
    assert!(!storage_debug.contains("apple-refresh-token"));
    assert!(!storage_debug.contains("provider-pkce-verifier"));
}

#[tokio::test]
async fn apple_callback_provider_error_redirects_to_client_without_code() {
    let id_token = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "unused-subject",
        aud: APPLE_CLIENT_ID,
        exp: (Utc::now() + Duration::minutes(10)).timestamp(),
        iat: Utc::now().timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("unused@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    let (state, storage) = apple_app_state_with_storage(id_token);
    let runtime = state.runtime.clone();
    seed_apple_callback_state(&storage, &runtime).await;

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/apple/callback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("error=access_denied&state=raw-provider-state"))
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
    let parsed = Url::parse(location).expect("client redirect");
    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("error").map(String::as_str),
        Some("access_denied")
    );
    assert_eq!(query.get("state").map(String::as_str), Some("client-state"));
    assert!(query.get("code").is_none());
    assert!(storage
        .query_prefix(&["oauth:code"])
        .await
        .expect("query_prefix auth codes")
        .is_empty());
}

#[tokio::test]
async fn apple_callback_internal_code_exchanges_through_token_endpoint() {
    let now = Utc::now();
    let id_token = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "apple-token-subject",
        aud: APPLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("token-apple@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    let (state, storage) = apple_app_state_with_storage(id_token);
    let runtime = state.runtime.clone();
    seed_apple_callback_state(&storage, &runtime).await;

    let app = create_router(state);
    let callback_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/apple/callback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("code=apple-code&state=raw-provider-state"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(callback_response.status(), StatusCode::SEE_OTHER);
    let callback_location = callback_response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("callback location");
    let callback_url = Url::parse(callback_location).expect("callback redirect url");
    let callback_query: HashMap<_, _> = callback_url.query_pairs().into_owned().collect();
    let internal_code = callback_query.get("code").expect("internal code");

    let token_body = format!(
        "grant_type=authorization_code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&code={internal_code}&code_verifier=client-code-verifier"
    );
    let token_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(token_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(token_response.status(), StatusCode::OK);
    let bytes = to_bytes(token_response.into_body(), 1024 * 1024)
        .await
        .expect("token response body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("token json");
    assert!(body["access_token"].as_str().is_some());
    assert!(body["id_token"].as_str().is_some());
    assert_eq!(body["token_type"], "Bearer");
    let response_debug = body.to_string();
    assert!(!response_debug.contains("apple-access-token"));
    assert!(!response_debug.contains("apple-code"));
}

#[tokio::test]
async fn apple_callback_rejects_missing_or_unknown_state() {
    let id_token = sign_apple_id_token(TestAppleClaims {
        iss: APPLE_ISSUER,
        sub: "unused-subject",
        aud: APPLE_CLIENT_ID,
        exp: (Utc::now() + Duration::minutes(10)).timestamp(),
        iat: Utc::now().timestamp(),
        nonce: PROVIDER_NONCE,
        email: Some("unused@example.com"),
        email_verified: Some("true"),
        is_private_email: Some("false"),
    });
    let app = create_router(apple_app_state(id_token));

    let missing_state = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/apple/callback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("code=apple-code"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_state.status(), StatusCode::BAD_REQUEST);

    let unknown_state = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/apple/callback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("code=apple-code&state=unknown-provider-state"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_state.status(), StatusCode::BAD_REQUEST);
}

fn validation(now: chrono::DateTime<Utc>) -> AppleIdTokenValidation<'static> {
    AppleIdTokenValidation {
        issuer: APPLE_ISSUER,
        client_id: APPLE_CLIENT_ID,
        nonce: PROVIDER_NONCE,
        now,
    }
}

fn write_client_config(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "irongate-apple-callback-client-config-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&path, contents).expect("write client config");
    path
}

fn runtime_with_apple_config() -> Arc<RuntimeAuthConfig> {
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
        (
            "AUTH_APPLE_CLIENT_ID".to_string(),
            APPLE_CLIENT_ID.to_string(),
        ),
        ("AUTH_APPLE_TEAM_ID".to_string(), "TEAMID1234".to_string()),
        ("AUTH_APPLE_KEY_ID".to_string(), "KEYID12345".to_string()),
        (
            "AUTH_APPLE_PRIVATE_KEY_SECRET".to_string(),
            "AUTH_APPLE_PRIVATE_KEY".to_string(),
        ),
        (
            "AUTH_APPLE_PRIVATE_KEY".to_string(),
            apple_signer.signing_key().private_key_pem.clone(),
        ),
    ]);

    Arc::new(
        RuntimeAuthConfig::from_env_map(&env, |name| env.get(name).cloned())
            .expect("runtime config"),
    )
}

fn apple_app_state(id_token: String) -> AppState {
    apple_app_state_with_storage(id_token).0
}

fn apple_app_state_with_storage(id_token: String) -> (AppState, TestStorage) {
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".to_string());
    let storage = TestStorage::new();
    let state = AppState {
        store: AuthStore::new(storage.clone()),
        config: Arc::new(config),
        runtime: runtime_with_apple_config(),
        email_sender: Arc::new(NoopEmailSender::default()),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(FakeAppleOidcClient {
            id_token: Arc::new(id_token),
        }),
    };
    (state, storage)
}

async fn seed_apple_callback_state(storage: &TestStorage, runtime: &Arc<RuntimeAuthConfig>) {
    let store = AuthStore::new(storage.clone());
    let session_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        "raw-authorize-session",
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
                code_challenge: Some(generate_challenge("client-code-verifier")),
                code_challenge_method: Some("S256".to_string()),
                selected_provider: Some("apple".to_string()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .expect("create authorize session");

    let state_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::ProviderState,
        "raw-provider-state",
    );
    store
        .create_provider_state(
            &state_digest,
            ProviderStateRecord {
                session_lookup_digest: session_digest,
                provider: "apple".to_string(),
                pkce_verifier: "provider-pkce-verifier".to_string(),
                nonce: PROVIDER_NONCE.to_string(),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .expect("create provider state");
}

fn jwks() -> AppleJwks {
    AppleJwks {
        keys: vec![AppleJwk {
            kty: "RSA".to_string(),
            kid: Some(TEST_KEY_ID.to_string()),
            use_: Some("sig".to_string()),
            alg: Some("RS256".to_string()),
            n: Some(TEST_RSA_N.to_string()),
            e: Some(TEST_RSA_E.to_string()),
        }],
    }
}

fn sign_apple_id_token(claims: TestAppleClaims<'_>) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KEY_ID.to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).expect("rsa key"),
    )
    .expect("sign apple token")
}

fn apple_identity_properties(email: Option<&str>) -> serde_json::Value {
    json!({
        "provider": "apple",
        "issuer": APPLE_ISSUER,
        "email": email,
        "email_verified": email.is_some(),
        "is_private_email": false
    })
}
