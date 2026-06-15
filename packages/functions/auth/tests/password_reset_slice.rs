use chrono::{Duration, Utc};
use irongate::config::email::EmailConfig;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::email::{render_password_reset_email, PasswordResetEmailInput};
use irongate::store::keys::StoreKey;
use irongate::store::records::PasswordResetRecord;
use irongate::store::AuthStore;
use irongate::StorageAdapter;

mod support;
use support::TestStorage;

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
