use super::*;
use irongate::storage::TransactOperation;
use irongate::store::keys::StoreKey;
use oidc_validation::identity_state;
use provider_callback::{issues_code, Failure, FaultStorage};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use transaction_gate::PausedStorage;

fn callback_request() -> Request<Body> {
    let profile = json!({
        "email": "unsigned@example.com",
        "sub": "unsigned-subject",
        "name": {"firstName": "Unsigned", "lastName": "Profile"}
    })
    .to_string();
    let form = serde_urlencoded::to_string([
        ("code", "apple-code"),
        ("state", "raw-provider-state"),
        ("user", profile.as_str()),
    ])
    .unwrap();
    Request::builder()
        .method("POST")
        .uri("/apple/callback")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap()
}

fn signed_token(email: Option<&str>) -> String {
    signed_token_for("provider-subject", email)
}

fn signed_token_for(subject: &str, email: Option<&str>) -> String {
    let mut claims =
        oidc_validation::claims(APPLE_ISSUER, APPLE_CLIENT_ID, PROVIDER_NONCE, Utc::now());
    claims["sub"] = json!(subject);
    if let Some(email) = email {
        claims["email"] = json!(email);
        claims["email_verified"] = json!(true);
    }
    claims["is_private_email"] = json!(true);
    claims["roles"] = json!(["untrusted-extra-claim"]);
    oidc_validation::sign(
        &claims,
        &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap(),
        Some(TEST_KEY_ID),
    )
}

async fn assert_issuance_error(response: axum::response::Response, conditional: bool) {
    assert_eq!(
        response.status(),
        if conditional {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    );
    assert!(!response.headers().contains_key(LOCATION));
    assert!(!response.headers().contains_key("set-cookie"));
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(
        body,
        if conditional {
            json!({"error": "invalid_grant", "error_description": "provider sign-in is no longer valid"})
        } else {
            json!({"error": "server_error", "error_description": "provider sign-in unavailable"})
        }
    );
}

async fn assert_consumed_and_replay_rejected(app: axum::Router, storage: &TestStorage) {
    for prefix in ["provider:state", "oauth:session"] {
        assert!(storage.query_prefix(&[prefix]).await.unwrap().is_empty());
    }
    let before = identity_state(storage).await;
    let response = app.oneshot(callback_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!response.headers().contains_key(LOCATION));
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["error"], "invalid_request");
    assert_eq!(
        identity_state(storage).await,
        before,
        "replay must not issue or change credentials"
    );
}

#[tokio::test]
async fn signed_email_changes_and_unsigned_profile_do_not_move_the_subject() {
    let (mut state, storage) = apple_app_state_with_storage(String::new());
    let original = seed_identity(&state.store, "original@example.com").await;
    let identity_digest = apple_identity_digest(LOOKUP_SECRET, APPLE_ISSUER, "provider-subject");
    for email in [
        Some("original@example.com"),
        Some("changed@example.com"),
        None,
    ] {
        state.apple_client = Arc::new(FakeAppleOidcClient {
            id_token: Arc::new(signed_token(email)),
        });
        seed_apple_callback_state(&storage, &state.runtime).await;
        let app = create_router(state.clone());
        let response = app.clone().oneshot(callback_request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(!response.headers().contains_key("set-cookie"));
        let url = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
        assert_eq!(
            url.as_str().split('?').next().unwrap(),
            "https://app.example.com/auth/callback"
        );
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.len(), 2, "only code and client state are returned");
        assert_eq!(query["state"], "client-state");
        let digest = lookup_digest(
            LOOKUP_SECRET,
            LookupFamily::AuthorizationCode,
            &query["code"],
        );
        let key = StoreKey::authorization_code(&digest);
        let code = storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap();
        assert_eq!(code["subject"], original.as_str());
        assert_eq!(code["credential_generation"], 0);
        assert_eq!(
            code["properties"],
            json!({
                "provider": "apple", "email": email, "email_verified": email.is_some()
            })
        );
        let identity = state
            .store
            .get_identity(IdentityProvider::Apple, &identity_digest)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(identity.subject.as_deref(), Some(original.as_str()));
        assert_eq!(
            identity.properties,
            Some(json!({
                "provider": "apple", "issuer": APPLE_ISSUER,
                "email": email, "email_verified": email.is_some(),
                    "is_private_email": true,
            }))
        );
        assert_eq!(storage.query_prefix(&["account:"]).await.unwrap().len(), 1);
        assert_consumed_and_replay_rejected(app, &storage).await;
    }
}

#[tokio::test]
async fn verified_apple_email_joins_password_owner_and_exchanges_code() {
    let email = "shared@example.com";
    let (state, storage) =
        apple_app_state_with_storage(signed_token_for("distinct-provider-subject", Some(email)));
    let password_subject = support::registration::register_verified_user(
        &state.store,
        &state.runtime,
        &support::registration::Mailbox::default(),
        email,
        "a password for the separate account",
    )
    .await;
    let provider_subject = seed_identity(&state.store, email).await;
    assert_eq!(provider_subject.as_str(), password_subject.as_str());
    seed_apple_callback_state(&storage, &state.runtime).await;
    let app = create_router(state.clone());
    let response = app.clone().oneshot(callback_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let url = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
    let code = url
        .query_pairs()
        .find(|(key, _)| key == "code")
        .unwrap()
        .1
        .into_owned();
    let digest = lookup_digest(LOOKUP_SECRET, LookupFamily::AuthorizationCode, &code);
    let record = AuthStore::new(storage.clone())
        .get_authorization_code(&digest)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.subject, password_subject.as_str());
    assert_eq!(record.properties["email"], email);
    assert_eq!(storage.query_prefix(&["account:"]).await.unwrap().len(), 1);
    let form = serde_urlencoded::to_string([
        ("grant_type", "authorization_code"),
        ("client_id", "web"),
        ("code", code.as_str()),
        ("redirect_uri", "https://app.example.com/auth/callback"),
        ("code_verifier", "client-code-verifier"),
    ])
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let tokens: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    let claims = state
        .runtime
        .signer
        .verify_access_token(
            tokens["access_token"].as_str().unwrap(),
            "https://auth.example.com",
            &state.runtime.access_token_audience,
        )
        .unwrap();
    assert_eq!(claims.sub, password_subject.as_str());
    assert!(tokens["id_token"].as_str().is_some());
}

#[tokio::test]
async fn only_signed_verified_email_can_claim_an_owner() {
    for (email, verified) in [
        (None, Some("true")),
        (Some("bad-address"), Some("true")),
        (Some("signed@example.com"), None),
        (Some("signed@example.com"), Some("false")),
    ] {
        let token = sign_apple_id_token(TestAppleClaims {
            iss: APPLE_ISSUER,
            sub: "unproven",
            aud: APPLE_CLIENT_ID,
            exp: (Utc::now() + Duration::minutes(5)).timestamp(),
            iat: Utc::now().timestamp(),
            nonce: PROVIDER_NONCE,
            email,
            email_verified: verified,
            is_private_email: Some("true"),
        });
        let (state, storage) = apple_app_state_with_storage(token);
        seed_apple_callback_state(&storage, &state.runtime).await;
        // The request contains unsigned user.email; it cannot fill missing signed authority.
        assert_issuance_error(
            create_router(state)
                .oneshot(callback_request())
                .await
                .unwrap(),
            true,
        )
        .await;
        assert!(identity_state(&storage).await.is_empty());
        assert!(storage
            .query_prefix(&["email:owner"])
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn prepare_and_finish_storage_failures_have_distinct_sanitized_responses() {
    for on_prepare in [true, false] {
        for failure in [
            Failure::Condition,
            Failure::Exists,
            Failure::Operational,
            Failure::Conflict,
        ] {
            let (mut state, storage) =
                apple_app_state_with_storage(signed_token(Some("signed@example.com")));
            seed_apple_callback_state(&storage, &state.runtime).await;
            let fault = Arc::new(FaultStorage {
                inner: storage.clone(),
                on_prepare,
                failure,
                failures: AtomicUsize::new(0),
            });
            state.store = AuthStore::new(fault.clone());
            let before = identity_state(&storage).await;
            let app = create_router(state);
            let response = app.clone().oneshot(callback_request()).await.unwrap();
            assert_issuance_error(
                response,
                matches!(failure, Failure::Condition | Failure::Exists),
            )
            .await;
            assert_eq!(
                fault.failures.load(Ordering::SeqCst),
                1,
                "no issuance retry"
            );
            assert_eq!(
                identity_state(&storage).await,
                before,
                "{on_prepare} {failure:?}"
            );
            assert_consumed_and_replay_rejected(app, &storage).await;
        }
    }
}

#[tokio::test]
async fn account_change_at_commit_rejects_without_metadata_or_code_writes() {
    let (mut state, storage) =
        apple_app_state_with_storage(signed_token(Some("changed@example.com")));
    let original = seed_identity(&state.store, "original@example.com").await;
    let store = state.store.clone();
    seed_apple_callback_state(&storage, &state.runtime).await;
    let before_callback = identity_state(&storage).await;
    let (paused, mut gate) = PausedStorage::when(storage.clone(), issues_code);
    state.store = AuthStore::new(paused);
    let app = create_router(state);
    let callback = tokio::spawn(app.clone().oneshot(callback_request()));
    let pending = gate.wait().await;
    assert_eq!(
        identity_state(&storage).await,
        before_callback,
        "callback must not write identity/account/code state before the issuance transaction"
    );
    store.disable_account(&original).await.unwrap();
    let before = identity_state(&storage).await;
    pending.commit();
    assert_issuance_error(callback.await.unwrap().unwrap(), true).await;
    assert_eq!(identity_state(&storage).await, before);
    assert_consumed_and_replay_rejected(app, &storage).await;
}

#[tokio::test]
async fn failed_dispatch_and_lost_response_do_not_claim_the_same_storage_outcome() {
    for committed in [false, true] {
        let (mut state, storage) =
            apple_app_state_with_storage(signed_token(Some("signed@example.com")));
        seed_apple_callback_state(&storage, &state.runtime).await;
        let (paused, mut gate) = PausedStorage::when(storage.clone(), issues_code);
        state.store = AuthStore::new(paused);
        let app = create_router(state);
        let callback = tokio::spawn(app.clone().oneshot(callback_request()));
        let pending = gate.wait().await;
        let writes: Vec<_> = pending
            .operations
            .iter()
            .filter_map(|operation| match operation {
                TransactOperation::Put { key, value, .. } => Some((key.clone(), value.clone())),
                _ => None,
            })
            .collect();
        assert!(writes.iter().any(|(key, _)| key[0] == "oauth:code"));
        assert!(writes.iter().any(|(key, _)| key[0].starts_with("account:")));
        assert!(writes.iter().any(|(key, _)| key[0] == "identity:apple"));
        assert!(writes
            .iter()
            .any(|(key, _)| key[0].starts_with("identity_by_subject:")));
        if committed {
            pending.lose_response();
        } else {
            pending.fail_before_dispatch();
        }
        assert_issuance_error(callback.await.unwrap().unwrap(), false).await;
        for (key, value) in writes {
            let parts: Vec<_> = key.iter().map(String::as_str).collect();
            assert_eq!(
                storage.get(&parts).await.unwrap(),
                committed.then_some(value)
            );
        }
        if !committed {
            assert!(identity_state(&storage).await.is_empty());
        }
        assert_consumed_and_replay_rejected(app, &storage).await;
    }
}
