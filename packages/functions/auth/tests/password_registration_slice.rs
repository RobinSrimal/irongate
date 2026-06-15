use irongate::core::passwords::{
    hash_password_for_storage, normalize_email, validate_password, PasswordPolicy,
};

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
