use chrono::{Duration, Utc};
use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::passwords::hash_password_for_storage;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::providers::password::{login_password_user, PasswordLoginInput, PasswordLoginStatus};
use irongate::store::records::{AuthorizationCodeRecord, AuthorizeSessionRecord};
use irongate::store::{AuthStore, IdentityProvider};
use irongate::StorageAdapter;
use serde_json::json;

mod support;
use support::TestStorage;

fn authorize_session_record(expires_at: chrono::DateTime<Utc>) -> AuthorizeSessionRecord {
    AuthorizeSessionRecord {
        client_id: "web".to_string(),
        redirect_uri: "https://app.example.com/auth/callback".to_string(),
        state: Some("state-123".to_string()),
        scope: "openid email".to_string(),
        oidc_nonce: Some("nonce-123".to_string()),
        code_challenge: Some("pkce-challenge".to_string()),
        code_challenge_method: Some("S256".to_string()),
        selected_provider: Some("password".to_string()),
        created_at: Utc::now(),
        expires_at,
    }
}

#[tokio::test]
async fn authorize_session_store_uses_hmac_keys_and_consumes_once() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let raw_session = "raw-authorize-session-secret";
    let session_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    let expires_at = Utc::now() + Duration::minutes(10);

    store
        .create_authorize_session(&session_digest, authorize_session_record(expires_at))
        .await
        .expect("create authorize session");

    let stored = storage
        .scan(&["oauth:session"])
        .await
        .expect("scan sessions");
    assert_eq!(stored.len(), 1);
    assert!(!stored[0].0.iter().any(|part| part.contains(raw_session)));

    let consumed = store
        .take_authorize_session(&session_digest)
        .await
        .expect("take session")
        .expect("session exists");
    assert_eq!(consumed.client_id, "web");
    assert_eq!(consumed.oidc_nonce.as_deref(), Some("nonce-123"));
    assert!(store
        .take_authorize_session(&session_digest)
        .await
        .expect("take session again")
        .is_none());
}

#[tokio::test]
async fn authorization_code_store_uses_hmac_key_and_stores_expiry() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let raw_code = "raw-authorization-code-secret";
    let code_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizationCode,
        raw_code,
    );
    let expires_at = Utc::now() + Duration::seconds(runtime.ttls.auth_code_seconds as i64);

    store
        .create_authorization_code(
            &code_digest,
            AuthorizationCodeRecord {
                client_id: "web".to_string(),
                redirect_uri: "https://app.example.com/auth/callback".to_string(),
                subject: "user_123".to_string(),
                subject_type: "user".to_string(),
                properties: json!({
                    "email": "user@example.com",
                    "email_verified": true,
                    "provider": "password"
                }),
                code_challenge: Some("pkce-challenge".to_string()),
                code_challenge_method: Some("S256".to_string()),
                scope: "openid email".to_string(),
                oidc_nonce: Some("nonce-123".to_string()),
                created_at: Utc::now(),
                expires_at,
            },
        )
        .await
        .expect("create authorization code");

    let stored = storage.scan(&["oauth:code"]).await.expect("scan codes");
    assert_eq!(stored.len(), 1);
    assert!(!stored[0].0.iter().any(|part| part.contains(raw_code)));
    assert_eq!(
        stored[0].1["expires_at"],
        serde_json::to_value(expires_at).unwrap()
    );
}

#[tokio::test]
async fn password_login_issues_redirect_code_for_verified_active_user() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let email = "user@example.com";
    let password = "correct horse battery staple";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let password_hash = hash_password_for_storage(password).expect("hash password");

    store
        .create_unverified_password_user(&email_digest, email, &password_hash)
        .await
        .expect("create password user");
    let subject = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify password user");

    let raw_session = "raw-login-session-secret";
    let session_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    store
        .create_authorize_session(
            &session_digest,
            authorize_session_record(Utc::now() + Duration::minutes(10)),
        )
        .await
        .expect("create authorize session");

    let outcome = login_password_user(
        &store,
        &runtime,
        PasswordLoginInput {
            session: raw_session,
            email,
            password,
        },
    )
    .await
    .expect("login password user");

    assert_eq!(outcome.status, PasswordLoginStatus::AuthorizationCodeIssued);
    let redirect = url::Url::parse(&outcome.redirect_uri).expect("redirect url");
    assert_eq!(
        redirect.as_str().split('?').next().unwrap(),
        "https://app.example.com/auth/callback"
    );
    assert_eq!(
        redirect
            .query_pairs()
            .find(|(name, _)| name == "state")
            .map(|(_, value)| value.into_owned()),
        Some("state-123".to_string())
    );
    let raw_code = redirect
        .query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, value)| value.into_owned())
        .expect("authorization code");
    assert!(!raw_code.is_empty());

    let code_records = storage.scan(&["oauth:code"]).await.expect("scan codes");
    assert_eq!(code_records.len(), 1);
    assert!(!code_records[0]
        .0
        .iter()
        .any(|part| part.contains(&raw_code)));
    assert_eq!(code_records[0].1["subject"], subject.as_str());
    assert!(store
        .take_authorize_session(&session_digest)
        .await
        .expect("session was consumed")
        .is_none());
}

#[tokio::test]
async fn password_login_wrong_password_does_not_consume_session() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage);
    let email = "user@example.com";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let password_hash =
        hash_password_for_storage("correct horse battery staple").expect("hash password");

    store
        .create_unverified_password_user(&email_digest, email, &password_hash)
        .await
        .expect("create password user");
    store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify password user");

    let raw_session = "raw-login-session-secret";
    let session_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    store
        .create_authorize_session(
            &session_digest,
            authorize_session_record(Utc::now() + Duration::minutes(10)),
        )
        .await
        .expect("create authorize session");

    let err = login_password_user(
        &store,
        &runtime,
        PasswordLoginInput {
            session: raw_session,
            email,
            password: "wrong horse battery staple",
        },
    )
    .await
    .expect_err("wrong password should fail");

    assert_eq!(err.to_string(), "invalid email or password");
    assert!(store
        .take_authorize_session(&session_digest)
        .await
        .expect("session should remain")
        .is_some());
}
