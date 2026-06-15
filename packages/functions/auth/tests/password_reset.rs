use chrono::{Duration, Utc};
use irongate::config::email::EmailConfig;
use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::passwords::hash_password_for_storage;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::password::verify_password;
use irongate::email::{
    render_password_reset_email, EmailDeliveryError, PasswordResetEmailInput, RenderedEmail,
    VerificationEmailSender,
};
use irongate::providers::password::{
    complete_password_reset, request_password_reset, PasswordResetCompleteError,
    PasswordResetCompleteInput, PasswordResetCompleteStatus, PasswordResetRequestInput,
    PasswordResetRequestStatus,
};
use irongate::store::keys::StoreKey;
use irongate::store::records::PasswordResetRecord;
use irongate::store::{AuthStore, IdentityProvider};
use irongate::storage::StorageAdapter;
use std::sync::{Arc, Mutex};

mod support;
use support::TestStorage;

#[derive(Clone, Default)]
struct FakeEmailSender {
    sent: Arc<Mutex<Vec<(String, RenderedEmail)>>>,
}

#[async_trait::async_trait]
impl VerificationEmailSender for FakeEmailSender {
    async fn send_verification_email(
        &self,
        to: &str,
        message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        self.sent
            .lock()
            .expect("sent lock")
            .push((to.to_string(), message));
        Ok("fake-reset-delivery".to_string())
    }
}

#[tokio::test]
async fn password_reset_secret_is_hmac_keyed_single_use_and_rejects_expired_records() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let reset_token = "raw-reset-token-with-high-entropy";
    let reset_digest = lookup_digest(
        b"0123456789abcdef0123456789abcdef",
        LookupFamily::PasswordReset,
        reset_token,
    );
    let expires_at = Utc::now() + Duration::minutes(10);

    store
        .create_password_reset(&reset_digest, "email_digest", "user_123", expires_at)
        .await
        .expect("create reset secret");

    let key = StoreKey::password_reset(&reset_digest);
    let stored = storage
        .get(&[key.pk(), key.sk()])
        .await
        .expect("get reset record")
        .expect("reset record");
    let record: PasswordResetRecord = serde_json::from_value(stored).expect("reset record json");
    assert_eq!(record.email_digest, "email_digest");
    assert_eq!(record.subject, "user_123");
    assert_eq!(record.purpose, "reset_password");
    assert_eq!(record.expires_at, expires_at);

    let raw_lookup = storage
        .get(&["password:reset", reset_token])
        .await
        .expect("raw reset lookup");
    assert!(raw_lookup.is_none());

    let consumed = store
        .consume_password_reset(&reset_digest)
        .await
        .expect("consume reset")
        .expect("reset exists");
    assert_eq!(consumed.email_digest, "email_digest");
    assert_eq!(consumed.subject, "user_123");

    assert!(store
        .consume_password_reset(&reset_digest)
        .await
        .expect("second consume")
        .is_none());

    store
        .create_password_reset(
            "expired_reset_digest",
            "email_digest",
            "user_123",
            Utc::now() - Duration::seconds(1),
        )
        .await
        .expect("create expired reset");
    assert!(store
        .consume_password_reset("expired_reset_digest")
        .await
        .expect("consume expired")
        .is_none());
}

#[test]
fn password_reset_email_template_renders_url_encoded_token_and_escaped_html() {
    let mut config = EmailConfig::for_tests();
    config.brand_name = "Acme <Auth>".to_string();
    config.support_email = Some("help@example.com".to_string());
    config.reset_subject = "Reset with Acme".to_string();

    let rendered = render_password_reset_email(PasswordResetEmailInput {
        config: &config,
        email: "user+reset@example.com",
        reset_token: "tok_abc+123",
        expires_minutes: 15,
    });

    assert_eq!(rendered.subject, "Reset with Acme");
    assert!(rendered.html.contains("Acme &lt;Auth&gt;"));
    assert!(!rendered.html.contains("Acme <Auth>"));
    assert!(rendered.html.contains("token=tok_abc%2B123"));
    assert!(rendered.text.contains("token=tok_abc%2B123"));
    assert!(rendered.text.contains("15 minutes"));
    assert!(rendered.text.contains("help@example.com"));
}

#[tokio::test]
async fn password_user_store_updates_hash_only_for_expected_verified_subject() {
    let store = AuthStore::new(TestStorage::new());
    let email = "user@example.com";
    let old_password = "correct horse battery staple";
    let new_password = "new correct horse battery staple";
    let email_digest = lookup_digest(
        b"0123456789abcdef0123456789abcdef",
        LookupFamily::Email,
        email,
    );
    let identity_digest = lookup_digest(
        b"0123456789abcdef0123456789abcdef",
        LookupFamily::PasswordIdentity,
        email,
    );
    let old_hash = hash_password_for_storage(old_password).expect("old hash");

    store
        .create_unverified_password_user(&email_digest, email, &old_hash)
        .await
        .expect("create password user");
    let subject = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            serde_json::json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify user");
    let new_hash = hash_password_for_storage(new_password).expect("new hash");

    store
        .update_password_hash(&email_digest, subject.as_str(), &new_hash)
        .await
        .expect("update password hash");

    let updated = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get password user")
        .expect("password user");
    assert!(updated.verified);
    assert_eq!(updated.subject.as_deref(), Some(subject.as_str()));
    let updated_hash = updated.password_hash.as_deref().expect("password hash");
    assert!(verify_password(new_password, updated_hash));
    assert!(!verify_password(old_password, updated_hash));

    let wrong_subject = store
        .update_password_hash(&email_digest, "user_wrong", &old_hash)
        .await;
    assert!(wrong_subject.is_err());
}

#[tokio::test]
async fn password_reset_request_is_generic_for_unknown_email_and_sends_nothing() {
    let runtime = RuntimeAuthConfig::for_tests();
    let store = AuthStore::new(TestStorage::new());
    let sender = FakeEmailSender::default();

    let outcome = request_password_reset(
        &store,
        &runtime,
        &sender,
        PasswordResetRequestInput {
            email: "unknown@example.com",
        },
    )
    .await
    .expect("request reset");

    assert_eq!(outcome.status, PasswordResetRequestStatus::ResetEmailSent);
    assert_eq!(sender.sent.lock().expect("sent lock").len(), 0);
}

#[tokio::test]
async fn password_reset_request_for_verified_active_account_sends_reset_email() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let sender = FakeEmailSender::default();
    let email = "user@example.com";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let hash = hash_password_for_storage("correct horse battery staple").expect("hash");
    store
        .create_unverified_password_user(&email_digest, email, &hash)
        .await
        .expect("create password user");
    let subject = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            serde_json::json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify user");

    let outcome = request_password_reset(
        &store,
        &runtime,
        &sender,
        PasswordResetRequestInput { email },
    )
    .await
    .expect("request reset");

    assert_eq!(outcome.status, PasswordResetRequestStatus::ResetEmailSent);
    let sent = sender.sent.lock().expect("sent lock");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, email);
    assert!(sent[0].1.html.contains("token="));

    let reset_records = storage.scan(&["password:reset"]).await.expect("scan resets");
    assert_eq!(reset_records.len(), 1);
    let record: PasswordResetRecord =
        serde_json::from_value(reset_records[0].1.clone()).expect("reset record");
    assert_eq!(record.email_digest, email_digest);
    assert_eq!(record.subject, subject.as_str());
    let keys = format!("{:?}", reset_records);
    assert!(!keys.contains("token="));
}

#[tokio::test]
async fn completing_password_reset_updates_password_and_consumes_token_once() {
    let runtime = RuntimeAuthConfig::for_tests();
    let store = AuthStore::new(TestStorage::new());
    let email = "user@example.com";
    let old_password = "correct horse battery staple";
    let new_password = "new correct horse battery staple";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let old_hash = hash_password_for_storage(old_password).expect("old hash");
    store
        .create_unverified_password_user(&email_digest, email, &old_hash)
        .await
        .expect("create password user");
    let subject = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            serde_json::json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify user");
    let reset_token = "raw-reset-token";
    let reset_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordReset,
        reset_token,
    );
    store
        .create_password_reset(
            &reset_digest,
            &email_digest,
            subject.as_str(),
            Utc::now() + Duration::minutes(10),
        )
        .await
        .expect("create reset");

    let outcome = complete_password_reset(
        &store,
        &runtime,
        PasswordResetCompleteInput {
            token: reset_token,
            new_password,
        },
    )
    .await
    .expect("complete reset");

    assert_eq!(outcome.status, PasswordResetCompleteStatus::PasswordReset);
    let updated = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get user")
        .expect("user");
    let updated_hash = updated.password_hash.as_deref().expect("password hash");
    assert!(verify_password(new_password, updated_hash));
    assert!(!verify_password(old_password, updated_hash));

    let reuse = complete_password_reset(
        &store,
        &runtime,
        PasswordResetCompleteInput {
            token: reset_token,
            new_password: "another correct horse battery staple",
        },
    )
    .await;
    assert!(matches!(
        reuse,
        Err(PasswordResetCompleteError::InvalidResetToken)
    ));
}
