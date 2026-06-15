use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::api::admin::{create_admin_router, AdminAppState};
use irongate::config::account_lifecycle::AccountLifecycleConfig;
use irongate::config::environment::RuntimeAuthConfig;
use irongate::core::passwords::hash_password_for_storage;
use irongate::core::subjects::Subject;
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::providers::password::{register_password_user, PasswordRegistrationInput};
use irongate::storage::StorageAdapter;
use irongate::store::keys::StoreKey;
use irongate::store::records::{AccountStatus, IdentityStatus, RefreshTokenFamilyRecord};
use irongate::store::refresh::CreateRefreshTokenInput;
use irongate::store::{AuthStore, DeletedIdentityReusePolicy, IdentityProvider};
use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayRequestAuthorizer, ApiGatewayRequestAuthorizerIamDescription,
    ApiGatewayV2httpRequestContext,
};
use lambda_http::request::RequestContext;
use serde_json::{json, Value};
use tower::ServiceExt;

mod support;
use support::{NoopEmailSender, TestStorage};

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn admin_state_with_storage() -> (AdminAppState, TestStorage) {
    let storage = TestStorage::new();
    let state = AdminAppState {
        store: AuthStore::new(storage.clone()),
        lifecycle: AccountLifecycleConfig::default(),
    };
    (state, storage)
}

fn iam_context() -> RequestContext {
    let mut context = ApiGatewayV2httpRequestContext::default();
    context.authorizer = Some(ApiGatewayRequestAuthorizer {
        iam: Some(ApiGatewayRequestAuthorizerIamDescription {
            account_id: Some("123456789012".to_string()),
            caller_id: Some("admin-caller".to_string()),
            user_arn: Some("arn:aws:iam::123456789012:role/irongate-admin".to_string()),
            user_id: Some("admin-user".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    });
    RequestContext::ApiGatewayV2(context)
}

fn admin_request(method: &str, uri: String, with_iam: bool) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if with_iam {
        builder = builder.extension(iam_context());
    }
    builder.body(Body::empty()).unwrap()
}

async fn create_password_account_with_refresh_and_reset(
    store: &AuthStore,
) -> (String, String, String, String, String) {
    let email = "user@example.com";
    let email_digest = "email-digest-delete-slice";
    let identity_digest = "password-identity-digest-delete-slice";
    let reset_digest = "reset-digest-delete-slice";
    let password_hash =
        hash_password_for_storage("correct horse battery staple").expect("password hash");

    store
        .create_unverified_password_user(email_digest, email, &password_hash)
        .await
        .expect("create password user");
    let subject = store
        .verify_password_user_with_identity(
            email_digest,
            IdentityProvider::Password,
            identity_digest,
            json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify password user");
    let refresh = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "web".to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": email, "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");
    store
        .create_password_reset(
            reset_digest,
            email_digest,
            subject.as_str(),
            Utc::now() + Duration::minutes(15),
        )
        .await
        .expect("create reset");

    (
        subject.as_str().to_string(),
        refresh.family_id,
        email_digest.to_string(),
        identity_digest.to_string(),
        reset_digest.to_string(),
    )
}

#[tokio::test]
async fn delete_account_tombstones_auth_owned_state_and_revokes_sessions() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let (subject, family_id, email_digest, identity_digest, reset_digest) =
        create_password_account_with_refresh_and_reset(&store).await;
    let subject_ref = Subject::from_persisted(subject.clone());

    let outcome = store
        .delete_account(&subject_ref, DeletedIdentityReusePolicy::AfterRetention, 30)
        .await
        .expect("delete account");

    assert_eq!(outcome.account.status, AccountStatus::Deleted);
    assert_eq!(outcome.deleted_identities, 1);
    assert_eq!(outcome.deleted_password_users, 1);
    assert_eq!(outcome.deleted_password_secrets, 1);
    assert_eq!(outcome.revoked_refresh_families, 1);
    assert!(!store
        .is_active_account(&subject_ref)
        .await
        .expect("active check"));

    let identity = store
        .get_identity(IdentityProvider::Password, &identity_digest)
        .await
        .expect("get identity")
        .expect("identity");
    assert_eq!(identity.status, IdentityStatus::Deleted);
    assert_eq!(identity.subject.as_deref(), None);
    assert_eq!(identity.properties, None);
    assert!(identity.deleted_at.is_some());
    assert!(identity.reusable_after.is_some());

    let password_user = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get password user")
        .expect("password user");
    assert_eq!(password_user.email.as_deref(), None);
    assert_eq!(password_user.password_hash.as_deref(), None);
    assert_eq!(password_user.subject.as_deref(), None);
    assert!(!password_user.verified);
    assert!(password_user.deleted_at.is_some());

    let reset_key = StoreKey::password_reset(&reset_digest);
    assert!(storage
        .get(&[reset_key.pk(), reset_key.sk()])
        .await
        .expect("get reset")
        .is_none());

    let family_key = StoreKey::refresh_family(&family_id);
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
async fn deleted_identity_reuse_policy_uses_tombstone_without_old_subject() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage);
    let original = store
        .create_account_with_identity(
            IdentityProvider::Google,
            "google-identity-digest-delete-slice",
            json!({"email": "user@example.com"}),
        )
        .await
        .expect("create account");

    store
        .delete_account(&original, DeletedIdentityReusePolicy::Immediate, 30)
        .await
        .expect("delete account");

    let tombstone = store
        .get_identity(
            IdentityProvider::Google,
            "google-identity-digest-delete-slice",
        )
        .await
        .expect("get identity")
        .expect("identity");
    assert_eq!(tombstone.subject.as_deref(), None);

    let replacement = store
        .reuse_deleted_identity(
            IdentityProvider::Google,
            "google-identity-digest-delete-slice",
            DeletedIdentityReusePolicy::Immediate,
            json!({"email": "user@example.com"}),
        )
        .await
        .expect("reuse identity");

    assert_ne!(original.as_str(), replacement.as_str());
}

#[tokio::test]
async fn deleted_identity_never_policy_blocks_later_reuse_even_if_runtime_policy_changes() {
    let store = AuthStore::new(TestStorage::new());
    let original = store
        .create_account_with_identity(
            IdentityProvider::Google,
            "google-never-reuse-delete-slice",
            json!({"email": "user@example.com"}),
        )
        .await
        .expect("create account");

    store
        .delete_account(&original, DeletedIdentityReusePolicy::Never, 30)
        .await
        .expect("delete account");

    let reused = store
        .reuse_deleted_identity(
            IdentityProvider::Google,
            "google-never-reuse-delete-slice",
            DeletedIdentityReusePolicy::Immediate,
            json!({"email": "user@example.com"}),
        )
        .await;

    assert!(reused.is_err());
}

#[tokio::test]
async fn password_registration_can_reuse_deleted_identity_after_immediate_policy() {
    let mut runtime = RuntimeAuthConfig::for_tests();
    runtime.account_lifecycle =
        AccountLifecycleConfig::from_values("immediate", 30).expect("lifecycle config");
    let storage = TestStorage::new();
    let store = AuthStore::new(storage);
    let email = "reuse@example.com";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let identity_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::PasswordIdentity,
        email,
    );
    let first_hash = hash_password_for_storage("correct horse battery staple").expect("first hash");

    store
        .create_unverified_password_user(&email_digest, email, &first_hash)
        .await
        .expect("create password user");
    let original = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify first account");
    store
        .delete_account(&original, DeletedIdentityReusePolicy::Immediate, 30)
        .await
        .expect("delete account");

    register_password_user(
        &store,
        &runtime,
        &NoopEmailSender,
        PasswordRegistrationInput {
            email,
            password: "another correct horse battery staple",
        },
    )
    .await
    .expect("re-register password user");

    let recreated = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get password user")
        .expect("password user");
    assert_eq!(recreated.email.as_deref(), Some(email));
    assert!(recreated.password_hash.is_some());
    assert_eq!(recreated.subject.as_deref(), None);
    assert!(recreated.deleted_at.is_none());

    let replacement = store
        .verify_password_user_with_identity(
            &email_digest,
            IdentityProvider::Password,
            &identity_digest,
            json!({"email": email, "email_verified": true}),
        )
        .await
        .expect("verify replacement account");

    assert_ne!(original.as_str(), replacement.as_str());
}

#[tokio::test]
async fn admin_delete_route_is_iam_protected_and_returns_sanitized_deleted_state() {
    let (state, storage) = admin_state_with_storage();
    let store = AuthStore::new(storage);
    let (subject, _, _, _, _) = create_password_account_with_refresh_and_reset(&store).await;
    let app = create_admin_router(state);

    let missing_iam = app
        .clone()
        .oneshot(admin_request(
            "POST",
            format!("/_admin/users/{subject}/delete"),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(missing_iam.status(), StatusCode::FORBIDDEN);

    let response = app
        .oneshot(admin_request(
            "POST",
            format!("/_admin/users/{subject}/delete"),
            true,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("response body"),
    )
    .expect("admin delete json");
    assert_eq!(body["subject"], subject);
    assert_eq!(body["status"], "deleted");
    assert!(body["deleted_at"].as_str().is_some());
    assert_eq!(body["deleted_identities"], 1);
    assert_eq!(body["deleted_password_users"], 1);
    assert_eq!(body["deleted_password_secrets"], 1);
    assert_eq!(body["revoked_refresh_families"], 1);
    assert!(body.get("email").is_none());
    assert!(body.get("password_hash").is_none());
    assert!(body.get("identities").is_none());
    assert!(body.get("refresh_tokens").is_none());
    assert!(body.get("value").is_none());
}
