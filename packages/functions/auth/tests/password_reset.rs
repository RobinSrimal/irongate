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
use irongate::storage::StorageAdapter;
use irongate::store::keys::StoreKey;
use irongate::store::records::PasswordResetRecord;
use irongate::store::{AuthStore, IdentityProvider};
use std::sync::{Arc, Mutex};

mod support;
use support::TestStorage;
#[allow(dead_code)]
#[path = "support/recovery.rs"]
mod recovery;

#[tokio::test]
async fn reset_preparation_preserves_hmac_proof_until_password_commit() {
    let storage = TestStorage::new();
    let f = recovery::Fixture::new(Arc::new(storage.clone()), "recovery@example.com").await;
    let token = f.request().await;
    let key = f.reset_key(&token);
    let value = storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap();
    let proof: PasswordResetRecord = serde_json::from_value(value.clone()).unwrap();
    assert!(proof.email_digest == f.email_digest && proof.subject == f.subject);
    assert_eq!(proof.credential_generation, 0);
    assert!(storage
        .get(&["password:reset", &token])
        .await
        .unwrap()
        .is_none());
    let prepared = f
        .store
        .prepare_password_reset(&f.digest(&token))
        .await
        .unwrap()
        .unwrap();
    assert!(storage.get(&[key.pk(), key.sk()]).await.unwrap() == Some(value));
    assert!(f
        .store
        .finish_password_reset(prepared, &hash_password_for_storage(recovery::NEW).unwrap())
        .await
        .unwrap());
    assert!(f.password_is(recovery::NEW).await);
    assert_eq!(f.generation().await, 1);
    assert!(f
        .store
        .prepare_password_reset(&f.digest(&token))
        .await
        .unwrap()
        .is_none());
    let index = StoreKey::password_reset_by_subject(&f.subject, &f.digest(&token));
    assert!(storage
        .get(&[index.pk(), index.sk()])
        .await
        .unwrap()
        .is_none());

    f.store
        .create_password_reset(
            "expired",
            &f.email_digest,
            &f.subject,
            Utc::now() - Duration::hours(1),
        )
        .await
        .unwrap();
    assert!(f
        .store
        .prepare_password_reset("expired")
        .await
        .unwrap()
        .is_none());
}

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
    {
        let sent = sender.sent.lock().expect("sent lock");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, email);
        assert!(sent[0].1.html.contains("token="));
    }

    let reset_records = storage
        .query_prefix(&["password:reset"])
        .await
        .expect("query_prefix resets");
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
