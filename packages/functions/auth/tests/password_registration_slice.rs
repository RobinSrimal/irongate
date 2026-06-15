use irongate::config::email::EmailConfig;
use irongate::core::passwords::{
    hash_password_for_storage, normalize_email, validate_password, PasswordPolicy,
};
use irongate::email::{render_verification_email, VerificationEmailInput};

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
