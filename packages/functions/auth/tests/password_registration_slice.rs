use irongate::config::email::EmailConfig;
use irongate::core::passwords::{
    hash_password_for_storage, normalize_email, validate_password, PasswordPolicy,
};
use irongate::email::{
    build_resend_email_request, render_verification_email, EmailDeliveryError, RenderedEmail,
    VerificationEmailInput, VerificationEmailSender,
};
use std::sync::{Arc, Mutex};

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
