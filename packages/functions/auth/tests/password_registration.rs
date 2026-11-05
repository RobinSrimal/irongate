use chrono::{Duration, Utc};
use irongate::config::email::EmailConfig;
use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::passwords::{
    hash_password_for_storage, normalize_email, validate_password, PasswordPolicy,
};
use irongate::core::subjects::Subject;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::crypto::password::verify_password;
use irongate::email::{
    build_resend_email_request, render_verification_email, EmailDeliveryError, RenderedEmail,
    VerificationEmailInput, VerificationEmailSender,
};
use irongate::providers::password::{
    register_password_user, verify_password_email, PasswordRegistrationError,
    PasswordRegistrationInput, PasswordRegistrationStatus, PasswordVerificationError,
    PasswordVerificationInput, PasswordVerificationStatus,
};
use irongate::storage::StorageAdapter;
use irongate::store::keys::StoreKey;
use irongate::store::{AuthStore, IdentityProvider};
use serde_json::json;

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

const PASSWORD: &str = "correct horse battery staple";
const OTHER: &str = "some other secure password";
const EMAIL: &str = "user@example.com";

fn digest(runtime: &RuntimeAuthConfig, family: LookupFamily, value: &str) -> String {
    lookup_digest(runtime.lookup_secret.as_bytes(), family, value)
}

#[tokio::test]
async fn email_only_registration_and_resends_never_install_a_password() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let mailbox = support::registration::Mailbox::default();
    let key = digest(&runtime, LookupFamily::Email, EMAIL);
    let mut binding = None;
    for _ in 0..2 {
        let outcome = register_password_user(
            &store,
            &runtime,
            &mailbox,
            PasswordRegistrationInput {
                email: " User@Example.COM ",
            },
        )
        .await
        .unwrap();
        assert_eq!(
            outcome.status,
            PasswordRegistrationStatus::VerificationRequired
        );
        assert!(outcome.authorization_code.is_none() && outcome.access_token.is_none());
        let user = store
            .get_password_user_by_email_digest(&key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.email.as_deref(), Some(EMAIL));
        assert!(user.subject.is_none() && user.password_hash.is_none());
        assert!(user.password_hash_updated_at.is_none() && !user.verified);
        assert!(user.registration_id.is_some());
        if let Some(previous) = &binding {
            assert_eq!(user.registration_id.as_ref(), Some(previous));
        }
        binding = user.registration_id;
        let proof_key = digest(&runtime, LookupFamily::EmailVerification, &mailbox.last());
        let proof = storage
            .get(&["password:verify", &proof_key])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(proof["registration_id"].as_str(), binding.as_deref());
        assert!(!proof.to_string().contains(&mailbox.last()));
    }
    assert_eq!(mailbox.len(), 2);
    assert!(storage.query_prefix(&["account"]).await.unwrap().is_empty());
    assert!(storage
        .query_prefix(&["identity:password"])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn only_proof_holder_sets_password_and_sibling_links_cannot_replace_it() {
    let runtime = RuntimeAuthConfig::for_tests();
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let mailbox = support::registration::Mailbox::default();
    register_password_user(
        &store,
        &runtime,
        &mailbox,
        PasswordRegistrationInput { email: EMAIL },
    )
    .await
    .unwrap();
    let first = mailbox.last();
    register_password_user(
        &store,
        &runtime,
        &mailbox,
        PasswordRegistrationInput { email: EMAIL },
    )
    .await
    .unwrap();
    let second = mailbox.last();
    let invalid = verify_password_email(
        &store,
        &runtime,
        PasswordVerificationInput {
            token: &first,
            new_password: "short",
        },
    )
    .await;
    assert!(matches!(
        invalid,
        Err(PasswordVerificationError::Password(_))
    ));
    let outcome = verify_password_email(
        &store,
        &runtime,
        PasswordVerificationInput {
            token: &first,
            new_password: PASSWORD,
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, PasswordVerificationStatus::Verified);
    assert!(outcome.authorization_code.is_none() && outcome.access_token.is_none());
    let user = store
        .get_password_user_by_email_digest(&digest(&runtime, LookupFamily::Email, EMAIL))
        .await
        .unwrap()
        .unwrap();
    assert!(user.verified && user.registration_id.is_none());
    assert_eq!(user.subject.as_deref(), Some(outcome.subject.as_str()));
    assert!(verify_password(
        PASSWORD,
        user.password_hash.as_deref().unwrap()
    ));
    let identity = store
        .get_identity(
            IdentityProvider::Password,
            &digest(&runtime, LookupFamily::PasswordIdentity, EMAIL),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(identity.subject.as_deref(), Some(outcome.subject.as_str()));
    assert_eq!(identity.properties.unwrap()["email_verified"], true);
    for token in [first, second] {
        assert!(matches!(
            verify_password_email(
                &store,
                &runtime,
                PasswordVerificationInput {
                    token: &token,
                    new_password: OTHER
                }
            )
            .await,
            Err(PasswordVerificationError::InvalidVerificationToken)
        ));
    }
    let latest = store
        .get_password_user_by_email_digest(&digest(&runtime, LookupFamily::Email, EMAIL))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.password_hash, user.password_hash);
    assert!(!verify_password(
        OTHER,
        latest.password_hash.as_deref().unwrap()
    ));
    assert!(matches!(
        register_password_user(
            &store,
            &runtime,
            &mailbox,
            PasswordRegistrationInput { email: EMAIL }
        )
        .await,
        Err(PasswordRegistrationError::EmailAlreadyRegistered)
    ));
    store
        .disable_account(&Subject::from_persisted(outcome.subject))
        .await
        .unwrap();
    assert!(matches!(
        register_password_user(
            &store,
            &runtime,
            &mailbox,
            PasswordRegistrationInput { email: EMAIL }
        )
        .await,
        Err(PasswordRegistrationError::EmailAlreadyRegistered)
    ));
}

#[tokio::test]
async fn invalid_proofs_and_ineligible_pending_records_cannot_create_credentials() {
    for case in [
        "missing",
        "purpose",
        "expiry",
        "binding",
        "missing_proof_binding",
        "missing_pending_binding",
        "missing_user",
        "subject",
        "hash",
        "deleted",
        "email",
    ] {
        let runtime = RuntimeAuthConfig::for_tests();
        let storage = TestStorage::new();
        let store = AuthStore::new(storage.clone());
        let mailbox = support::registration::Mailbox::default();
        register_password_user(
            &store,
            &runtime,
            &mailbox,
            PasswordRegistrationInput { email: EMAIL },
        )
        .await
        .unwrap();
        let token = mailbox.last();
        let proof_key = StoreKey::password_verification(&digest(
            &runtime,
            LookupFamily::EmailVerification,
            &token,
        ));
        let user_key = StoreKey::password_user(&digest(&runtime, LookupFamily::Email, EMAIL));
        let mut proof = storage
            .get(&[proof_key.pk(), proof_key.sk()])
            .await
            .unwrap()
            .unwrap();
        let mut user = storage
            .get(&[user_key.pk(), user_key.sk()])
            .await
            .unwrap()
            .unwrap();
        match case {
            "missing" => {
                storage
                    .remove(&[proof_key.pk(), proof_key.sk()])
                    .await
                    .unwrap();
            }
            "missing_user" => {
                storage
                    .remove(&[user_key.pk(), user_key.sk()])
                    .await
                    .unwrap();
            }
            "purpose" | "expiry" | "binding" | "missing_proof_binding" => {
                match case {
                    "purpose" => proof["purpose"] = json!("reset_password"),
                    "expiry" => proof["expires_at"] = json!(Utc::now() - Duration::seconds(1)),
                    "missing_proof_binding" => {
                        proof.as_object_mut().unwrap().remove("registration_id");
                    }
                    _ => proof["registration_id"] = json!("unrelated-registration"),
                }
                // Keep the physical item present to exercise logical expiry, not TTL cleanup.
                storage
                    .set(&[proof_key.pk(), proof_key.sk()], proof, None)
                    .await
                    .unwrap();
            }
            _ => {
                match case {
                    "subject" => user["subject"] = json!("user_unrelated"),
                    "hash" => user["password_hash"] = json!("preinstalled-password-hash"),
                    "missing_pending_binding" => {
                        user.as_object_mut().unwrap().remove("registration_id");
                    }
                    "deleted" => user["deleted_at"] = json!(Utc::now()),
                    _ => user["email"] = json!("different@example.com"),
                }
                storage
                    .set(&[user_key.pk(), user_key.sk()], user, None)
                    .await
                    .unwrap();
            }
        }
        let before = storage.query_prefix(&["password"]).await.unwrap();
        assert!(
            matches!(
                verify_password_email(
                    &store,
                    &runtime,
                    PasswordVerificationInput {
                        token: &token,
                        new_password: PASSWORD
                    }
                )
                .await,
                Err(PasswordVerificationError::InvalidVerificationToken)
            ),
            "{case}"
        );
        assert_eq!(
            storage.query_prefix(&["password"]).await.unwrap(),
            before,
            "{case}"
        );
        assert!(
            storage.query_prefix(&["account"]).await.unwrap().is_empty(),
            "{case}"
        );
    }
}

#[tokio::test]
async fn delivery_failure_keeps_pending_state_and_allows_a_new_link() {
    struct Unavailable;
    #[async_trait::async_trait]
    impl VerificationEmailSender for Unavailable {
        async fn send_verification_email(
            &self,
            _: &str,
            _: RenderedEmail,
        ) -> Result<String, EmailDeliveryError> {
            Err(EmailDeliveryError::ProviderStatus { status: 503 })
        }
    }
    let runtime = RuntimeAuthConfig::for_tests();
    let store = AuthStore::new(TestStorage::new());
    let result = register_password_user(
        &store,
        &runtime,
        &Unavailable,
        PasswordRegistrationInput { email: EMAIL },
    )
    .await;
    assert!(matches!(
        result,
        Err(PasswordRegistrationError::EmailDelivery(_))
    ));
    let key = digest(&runtime, LookupFamily::Email, EMAIL);
    let before = store
        .get_password_user_by_email_digest(&key)
        .await
        .unwrap()
        .unwrap();
    let mailbox = support::registration::Mailbox::default();
    register_password_user(
        &store,
        &runtime,
        &mailbox,
        PasswordRegistrationInput { email: EMAIL },
    )
    .await
    .unwrap();
    let after = store
        .get_password_user_by_email_digest(&key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.registration_id, after.registration_id);
    assert!(after.password_hash.is_none());
    verify_password_email(
        &store,
        &runtime,
        PasswordVerificationInput {
            token: &mailbox.last(),
            new_password: PASSWORD,
        },
    )
    .await
    .unwrap();
}
