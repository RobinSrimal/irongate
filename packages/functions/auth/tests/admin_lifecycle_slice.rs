use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use irongate::api::admin::{create_admin_router, AdminAppState};
use irongate::core::subjects::Subject;
use irongate::store::keys::StoreKey;
use irongate::store::records::{
    AccountRecord, AccountStatus, RefreshTokenFamilyRecord, RefreshTokenRecord,
};
use irongate::store::refresh::CreateRefreshTokenInput;
use irongate::store::{AuthStore, IdentityProvider};
use irongate::StorageAdapter;
use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayRequestAuthorizer, ApiGatewayRequestAuthorizerIamDescription,
    ApiGatewayV2httpRequestContext,
};
use lambda_http::request::RequestContext;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod support;
use support::TestStorage;

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn admin_state() -> AdminAppState<TestStorage> {
    AdminAppState {
        storage: Arc::new(TestStorage::new()),
    }
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

async fn create_subject_with_refresh(
    store: &AuthStore<TestStorage>,
    client_id: &str,
) -> (String, String) {
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            &format!("password-identity-digest-{client_id}"),
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let refresh = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: client_id.to_string(),
                subject: subject.as_str().to_string(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create refresh token");

    (subject.as_str().to_string(), refresh.family_id)
}

#[tokio::test]
async fn disable_account_marks_account_inactive_and_is_idempotent() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage);
    let subject = store
        .create_account_with_identity(
            IdentityProvider::Password,
            "password-identity-digest",
            json!({"email": "user@example.com", "email_verified": true}),
        )
        .await
        .expect("create account");

    let disabled = store
        .disable_account(&subject)
        .await
        .expect("disable account");

    assert_eq!(disabled.status, AccountStatus::Disabled);
    assert!(disabled.disabled_at.is_some());
    assert!(!store.is_active_account(&subject).await.expect("active check"));

    let disabled_again = store
        .disable_account(&subject)
        .await
        .expect("disable account again");
    assert_eq!(disabled_again.status, AccountStatus::Disabled);
    assert_eq!(disabled_again.disabled_at, disabled.disabled_at);
}

#[tokio::test]
async fn deleted_account_cannot_be_disabled_or_restored() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let subject = Subject::from_persisted("user_deleted_fixture".to_string());
    let deleted = AccountRecord {
        subject: subject.as_str().to_string(),
        status: AccountStatus::Deleted,
        created_at: Utc::now(),
        disabled_at: None,
        deleted_at: Some(Utc::now()),
    };
    let key = StoreKey::account(subject.as_str());
    storage
        .set(
            &[key.pk(), key.sk()],
            serde_json::to_value(deleted).expect("deleted account json"),
            None,
        )
        .await
        .expect("seed deleted account");

    let result = store.disable_account(&subject).await;

    assert!(result.is_err());
    let account = store
        .get_account(&subject)
        .await
        .expect("get account")
        .expect("account");
    assert_eq!(account.status, AccountStatus::Deleted);
    assert!(!store.is_active_account(&subject).await.expect("active check"));
}

#[tokio::test]
async fn revoke_refresh_tokens_for_subject_revokes_indexed_families() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let (subject, first_family_id) = create_subject_with_refresh(&store, "web").await;
    let second = store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                client_id: "mobile".to_string(),
                subject: subject.clone(),
                subject_type: "user".to_string(),
                scope: "openid email offline_access".to_string(),
                properties: json!({"email": "user@example.com", "email_verified": true}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .expect("create second refresh token");

    let revoked = store
        .revoke_refresh_tokens_for_subject(&subject)
        .await
        .expect("revoke subject tokens");

    assert_eq!(revoked, 2);
    for family_id in [first_family_id.as_str(), second.family_id.as_str()] {
        let key = StoreKey::refresh_family(family_id);
        let family_value = storage
            .get(&[key.pk(), key.sk()])
            .await
            .expect("get family")
            .expect("family");
        let family: RefreshTokenFamilyRecord =
            serde_json::from_value(family_value).expect("family json");
        assert!(family.revoked_at.is_some());
    }

    let second_key = StoreKey::refresh_token(&second.refresh_digest);
    let second_value = storage
        .get(&[second_key.pk(), second_key.sk()])
        .await
        .expect("get current refresh")
        .expect("current refresh");
    let second_record: RefreshTokenRecord =
        serde_json::from_value(second_value).expect("current refresh json");
    assert!(second_record.revoked_at.is_some());

    let revoked_again = store
        .revoke_refresh_tokens_for_subject(&subject)
        .await
        .expect("revoke subject tokens again");
    assert_eq!(revoked_again, 0);
}

#[tokio::test]
async fn admin_get_user_returns_sanitized_account_status() {
    let state = admin_state();
    let store = AuthStore::new((*state.storage).clone());
    let (subject, _) = create_subject_with_refresh(&store, "web").await;
    store
        .disable_account(&Subject::from_persisted(subject.clone()))
        .await
        .expect("disable account");

    let response = create_admin_router(state)
        .oneshot(admin_request(
            "GET",
            format!("/_admin/users/{subject}"),
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
    .expect("admin json");
    assert_eq!(body["subject"], subject);
    assert_eq!(body["status"], "disabled");
    assert!(body["disabled_at"].as_str().is_some());
    assert!(body.get("password_hash").is_none());
    assert!(body.get("email").is_none());
    assert!(body.get("identities").is_none());
    assert!(body.get("refresh_tokens").is_none());
    assert!(body.get("value").is_none());
}

#[tokio::test]
async fn admin_routes_reject_missing_iam_context_and_custom_admin_key() {
    let state = admin_state();
    let store = AuthStore::new((*state.storage).clone());
    let (subject, _) = create_subject_with_refresh(&store, "web").await;
    let app = create_admin_router(state);

    let missing_iam = app
        .clone()
        .oneshot(admin_request(
            "GET",
            format!("/_admin/users/{subject}"),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(missing_iam.status(), StatusCode::FORBIDDEN);

    let custom_key = Request::builder()
        .method("GET")
        .uri(format!("/_admin/users/{subject}"))
        .header("x-admin-key", "legacy-admin-key")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(custom_key).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_disable_revokes_subject_sessions() {
    let state = admin_state();
    let storage = state.storage.clone();
    let store = AuthStore::new((*storage).clone());
    let (subject, family_id) = create_subject_with_refresh(&store, "web").await;

    let response = create_admin_router(state)
        .oneshot(admin_request(
            "POST",
            format!("/_admin/users/{subject}/disable"),
            true,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let account = store
        .get_account(&Subject::from_persisted(subject.clone()))
        .await
        .expect("get account")
        .expect("account");
    assert_eq!(account.status, AccountStatus::Disabled);

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
async fn admin_revoke_sessions_does_not_disable_account() {
    let state = admin_state();
    let storage = state.storage.clone();
    let store = AuthStore::new((*storage).clone());
    let (subject, family_id) = create_subject_with_refresh(&store, "web").await;

    let response = create_admin_router(state)
        .oneshot(admin_request(
            "POST",
            format!("/_admin/users/{subject}/revoke-sessions"),
            true,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let account = store
        .get_account(&Subject::from_persisted(subject.clone()))
        .await
        .expect("get account")
        .expect("account");
    assert_eq!(account.status, AccountStatus::Active);

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
