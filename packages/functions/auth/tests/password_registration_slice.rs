use chrono::{Duration, Utc};
use irongate::config::email::EmailConfig;
use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::passwords::{
    hash_password_for_storage, normalize_email, validate_password, PasswordPolicy,
};
use irongate::core::subjects::Subject;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::email::{
    build_resend_email_request, render_verification_email, EmailDeliveryError, RenderedEmail,
    VerificationEmailInput, VerificationEmailSender,
};
use irongate::flows::password::{
    register_password_user, verify_password_email, PasswordRegistrationInput,
    PasswordRegistrationStatus, PasswordVerificationInput, PasswordVerificationStatus,
};
use irongate::store::{AuthStore, IdentityProvider};
use std::sync::{Arc, Mutex};

mod support;
use support::TestStorage;

#[test]
fn password_policy_rejects_short_and_long_passwords() {
    let policy = PasswordPolicy::default();

    assert!(validate_password("short", &policy).is_err());
    assert!(validate_password(&"a".repeat(129), &policy).is_err());
    assert!(validate_password("correct horse battery staple", &policy).is_ok());
}

#[test]
fn password_hash_is_argon2id_phc_and_does_not_store_plaintext() {
    let password = "correct horse battery staple";
    let hash = hash_password_for_storage(password).expect("password hash");

    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains(password));
}

#[test]
fn password_hash_rejects_passwords_outside_policy() {
    assert!(hash_password_for_storage("short").is_err());
    assert!(hash_password_for_storage(&"a".repeat(129)).is_err());
}

#[test]
fn email_normalization_is_deterministic_and_rejects_malformed_input() {
    assert_eq!(
        normalize_email("  User.Name+tag@Example.COM  ").expect("email"),
        "user.name+tag@example.com"
    );

    assert!(normalize_email("missing-at.example.com").is_err());
    assert!(normalize_email("@example.com").is_err());
    assert!(normalize_email("user@").is_err());
    assert!(normalize_email("user@example.com@extra").is_err());
}

#[test]
fn verification_email_template_renders_url_encoded_token_and_escaped_html() {
    let mut config = EmailConfig::for_tests();
    config.brand_name = "Acme <Auth>".to_string();
    config.support_email = Some("help@example.com".to_string());
    config.verify_subject = "Verify with Acme".to_string();

    let rendered = render_verification_email(VerificationEmailInput {
        config: &config,
        email: "user@example.com",
        verification_token: "tok_abc+123",
        expires_minutes: 15,
    });

    assert_eq!(rendered.subject, "Verify with Acme");
    assert!(rendered.html.contains("Acme &lt;Auth&gt;"));
    assert!(!rendered.html.contains("Acme <Auth>"));
    assert!(rendered.html.contains("token=tok_abc%2B123"));
    assert!(rendered.text.contains("token=tok_abc%2B123"));
    assert!(rendered.text.contains("15 minutes"));
    assert!(rendered.text.contains("help@example.com"));
}

#[test]
fn resend_email_payload_contains_message_but_not_api_key() {
    let mut config = EmailConfig::for_tests();
    config.reply_to = Some("reply@example.com".to_string());

    let rendered = render_verification_email(VerificationEmailInput {
        config: &config,
        email: "user@example.com",
        verification_token: "tok_abc+123",
        expires_minutes: 15,
    });
    let request = build_resend_email_request(&config, "user@example.com", &rendered);
    let body = serde_json::to_string(&request).expect("serialize resend request");

    assert_eq!(request.from, config.from);
    assert_eq!(request.to, vec!["user@example.com".to_string()]);
    assert_eq!(request.reply_to, Some("reply@example.com".to_string()));
    assert!(body.contains("token=tok_abc%2B123"));
    assert!(!body.contains(config.resend_api_key.expose()));
}

#[tokio::test]
async fn fake_email_sender_can_satisfy_delivery_contract_without_network() {
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
            Ok("fake-delivery-1".to_string())
        }
    }

    let sender = FakeEmailSender::default();
    let message = RenderedEmail {
        subject: "Verify".to_string(),
        html: "<p>Verify</p>".to_string(),
        text: "Verify".to_string(),
    };

    let delivery_id = sender
        .send_verification_email("user@example.com", message)
        .await
        .expect("delivery");

    assert_eq!(delivery_id, "fake-delivery-1");
    assert_eq!(sender.sent.lock().expect("sent lock").len(), 1);
}

#[tokio::test]
async fn password_user_store_creates_unverified_user_and_marks_verified() {
    let store = AuthStore::new(TestStorage::new());
    let subject = Subject::generate();

    store
        .create_unverified_password_user("email_digest", "user@example.com", "$argon2id$test-hash")
        .await
        .expect("create password user");

    let user = store
        .get_password_user_by_email_digest("email_digest")
        .await
        .expect("get password user")
        .expect("password user exists");

    assert_eq!(user.email, "user@example.com");
    assert_eq!(user.subject, None);
    assert_eq!(user.password_hash, "$argon2id$test-hash");
    assert!(!user.verified);

    store
        .mark_password_user_verified("email_digest", &subject)
        .await
        .expect("mark verified");

    let verified = store
        .get_password_user_by_email_digest("email_digest")
        .await
        .expect("get verified user")
        .expect("verified user exists");

    assert_eq!(verified.subject.as_deref(), Some(subject.as_str()));
    assert!(verified.verified);
}

#[tokio::test]
async fn email_verification_secret_is_single_use_and_rejects_expired_records() {
    let store = AuthStore::new(TestStorage::new());
    let expires_at = Utc::now() + Duration::minutes(10);

    store
        .create_email_verification("verification_digest", "email_digest", expires_at)
        .await
        .expect("create verification secret");

    let consumed = store
        .consume_email_verification("verification_digest")
        .await
        .expect("consume verification")
        .expect("verification exists");

    assert_eq!(consumed.email_digest, "email_digest");
    assert_eq!(consumed.expires_at, expires_at);
    assert!(store
        .consume_email_verification("verification_digest")
        .await
        .expect("second consume")
        .is_none());

    store
        .create_email_verification(
            "expired_verification_digest",
            "email_digest",
            Utc::now() - Duration::seconds(1),
        )
        .await
        .expect("create expired verification secret");

    assert!(store
        .consume_email_verification("expired_verification_digest")
        .await
        .expect("consume expired")
        .is_none());
}

#[tokio::test]
async fn password_registration_creates_unverified_user_and_sends_verification_email() {
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
            Ok("fake-delivery-1".to_string())
        }
    }

    let runtime = RuntimeAuthConfig::for_tests();
    let store = AuthStore::new(TestStorage::new());
    let sender = FakeEmailSender::default();

    let outcome = register_password_user(
        &store,
        &runtime,
        &sender,
        PasswordRegistrationInput {
            email: "  User@Example.COM  ",
            password: "correct horse battery staple",
        },
    )
    .await
    .expect("register password user");

    assert_eq!(
        outcome.status,
        PasswordRegistrationStatus::VerificationRequired
    );
    assert_eq!(outcome.delivery_id, "fake-delivery-1");
    assert!(outcome.authorization_code.is_none());
    assert!(outcome.access_token.is_none());

    let sent = sender.sent.lock().expect("sent lock");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "user@example.com");
    assert!(sent[0].1.html.contains("token="));
}

#[tokio::test]
async fn password_email_verification_creates_password_identity_without_auth_tokens() {
    let runtime = RuntimeAuthConfig::for_tests();
    let store = AuthStore::new(TestStorage::new());
    let email = "user@example.com";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let verification_token = "raw-verification-token";
    let verification_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        verification_token,
    );

    store
        .create_unverified_password_user(&email_digest, email, "$argon2id$test-hash")
        .await
        .expect("create user");
    store
        .create_email_verification(
            &verification_digest,
            &email_digest,
            Utc::now() + Duration::minutes(10),
        )
        .await
        .expect("create verification");

    let outcome = verify_password_email(
        &store,
        &runtime,
        PasswordVerificationInput {
            token: verification_token,
        },
    )
    .await
    .expect("verify email");

    assert_eq!(outcome.status, PasswordVerificationStatus::Verified);
    assert!(outcome.subject.starts_with("user_"));
    assert!(outcome.authorization_code.is_none());
    assert!(outcome.access_token.is_none());

    let password_user = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get password user")
        .expect("password user");
    assert!(password_user.verified);
    assert_eq!(
        password_user.subject.as_deref(),
        Some(outcome.subject.as_str())
    );

    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let identity = store
        .get_identity(IdentityProvider::Password, &identity_digest)
        .await
        .expect("get identity")
        .expect("identity");
    assert_eq!(identity.subject, outcome.subject);
    assert_eq!(identity.properties["email"], email);
    assert_eq!(identity.properties["email_verified"], true);
}
