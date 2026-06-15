use chrono::{Duration, Utc};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::store::keys::StoreKey;
use irongate::store::records::{RefreshTokenFamilyRecord, RefreshTokenRecord};
use irongate::store::refresh::{
    CreateRefreshTokenInput, RefreshTokenStoreError, RevokeRefreshTokenOutcome,
};
use irongate::store::{AuthStore, IdentityProvider};
use irongate::StorageAdapter;
use serde_json::json;

mod support;
use support::TestStorage;

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn create_refresh_token_uses_hmac_keys_and_no_raw_token_key() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let created = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    let expected_digest = lookup_digest(
        LOOKUP_SECRET,
        LookupFamily::RefreshToken,
        &created.raw_token,
    );
    assert_eq!(created.refresh_digest, expected_digest);

    let key = StoreKey::refresh_token(&created.refresh_digest);
    assert_ne!(key.sk(), created.raw_token);
    let stored = storage
        .get(&[key.pk(), key.sk()])
        .await
        .expect("get refresh record")
        .expect("refresh record");
    let record: RefreshTokenRecord = serde_json::from_value(stored).expect("refresh record json");
    assert_eq!(record.refresh_digest, created.refresh_digest);
    assert_eq!(record.family_id, created.family_id);
    assert_eq!(record.client_id, "web");
    assert_eq!(record.subject, subject.as_str());
    assert_eq!(record.scope, "openid email offline_access");
    assert!(record.revoked_at.is_none());

    let raw_key_record = storage
        .get(&["oauth:refresh", &created.raw_token])
        .await
        .expect("raw token lookup");
    assert!(raw_key_record.is_none());

    let family_key = StoreKey::refresh_family(&created.family_id);
    let family_value = storage
        .get(&[family_key.pk(), family_key.sk()])
        .await
        .expect("get refresh family")
        .expect("refresh family");
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(family_value).expect("refresh family json");
    assert_eq!(family.current_refresh_digest, created.refresh_digest);
    assert!(family.revoked_at.is_none());
}

#[tokio::test]
async fn rotate_refresh_token_replaces_once_and_revokes_family_on_reuse() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let first = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    let rotated = store
        .rotate_refresh_token(
            LOOKUP_SECRET,
            &first.raw_token,
            "web",
            Utc::now() + Duration::days(30),
        )
        .await
        .expect("rotate refresh token");

    assert_ne!(rotated.raw_token, first.raw_token);
    assert_ne!(rotated.refresh_digest, first.refresh_digest);
    assert_eq!(rotated.family_id, first.family_id);
    assert_eq!(rotated.subject, subject.as_str());
    assert_eq!(rotated.scope, "openid email offline_access");

    let first_key = StoreKey::refresh_token(&first.refresh_digest);
    let first_value = storage
        .get(&[first_key.pk(), first_key.sk()])
        .await
        .expect("get first refresh")
        .expect("first refresh");
    let first_record: RefreshTokenRecord =
        serde_json::from_value(first_value).expect("first refresh json");
    assert_eq!(first_record.replaced_by.as_deref(), Some(rotated.refresh_digest.as_str()));
    assert!(first_record.revoked_at.is_none());

    let reuse = store
        .rotate_refresh_token(
            LOOKUP_SECRET,
            &first.raw_token,
            "web",
            Utc::now() + Duration::days(30),
        )
        .await;
    assert!(matches!(reuse, Err(RefreshTokenStoreError::ReuseDetected)));

    let family_key = StoreKey::refresh_family(&first.family_id);
    let family_value = storage
        .get(&[family_key.pk(), family_key.sk()])
        .await
        .expect("get family")
        .expect("family");
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(family_value).expect("family json");
    assert!(family.revoked_at.is_some());
}

#[tokio::test]
async fn revoke_refresh_token_family_is_idempotent_and_client_bound() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let created = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    let wrong_client = store
        .revoke_refresh_token_family(LOOKUP_SECRET, &created.raw_token, "mobile")
        .await
        .expect("wrong-client revoke");
    assert_eq!(wrong_client, RevokeRefreshTokenOutcome::NotFound);

    let revoked = store
        .revoke_refresh_token_family(LOOKUP_SECRET, &created.raw_token, "web")
        .await
        .expect("revoke");
    assert_eq!(revoked, RevokeRefreshTokenOutcome::Revoked);

    let again = store
        .revoke_refresh_token_family(LOOKUP_SECRET, &created.raw_token, "web")
        .await
        .expect("revoke again");
    assert_eq!(again, RevokeRefreshTokenOutcome::AlreadyRevoked);

    let rotate = store
        .rotate_refresh_token(
            LOOKUP_SECRET,
            &created.raw_token,
            "web",
            Utc::now() + Duration::days(30),
        )
        .await;
    assert!(matches!(rotate, Err(RefreshTokenStoreError::Invalid)));
}
