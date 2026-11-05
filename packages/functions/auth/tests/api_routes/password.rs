use super::*;
use irongate::providers::password::{register_password_user, PasswordRegistrationInput};
use irongate::store::keys::StoreKey;
use serde_json::json;
use support::registration::{register_verified_user, Mailbox};

async fn post_json(app: axum::Router, path: &str, body: Value) -> axum::response::Response {
    super::admission::post(app, path, body.to_string(), None, true).await
}

async fn pending_token(state: &AppState, email: &str) -> String {
    let mailbox = Mailbox::default();
    register_password_user(
        &state.store,
        &state.runtime,
        &mailbox,
        PasswordRegistrationInput { email },
    )
    .await
    .expect("register pending email");
    mailbox.last()
}

#[tokio::test]
async fn password_register_route_returns_verification_required_without_tokens() {
    let (mut state, storage) = app_state_with_storage();
    let mailbox = Arc::new(Mailbox::default());
    state.email_sender = mailbox.clone();
    let runtime = state.runtime.clone();
    let app = create_router(state);
    let body = r#"{"email":"user@example.com"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("set-cookie").is_none());
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("json response");

    assert_eq!(body, json!({"status": "verification_required"}));
    assert_eq!(mailbox.len(), 1);

    let email_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::Email,
        "user@example.com",
    );
    let store = AuthStore::new(storage.clone());
    let password_user = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get password user")
        .expect("pending password user");

    assert!(!password_user.verified);
    assert!(password_user.registration_id.is_some());
    assert!(password_user.password_hash.is_none());
    assert!(password_user.password_hash_updated_at.is_none());
    assert!(password_user.subject.is_none());
    for prefix in ["account", "oauth:code", "oauth:session", "oauth:refresh"] {
        assert!(storage.query_prefix(&[prefix]).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn password_verify_route_returns_subject_without_tokens() {
    let (state, storage) = app_state_with_storage();
    let runtime = state.runtime.clone();
    let store = AuthStore::new(storage.clone());
    let email = "user@example.com";
    let token = pending_token(&state, email).await;
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let verification_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        &token,
    );
    let password = "owner chosen correct horse battery staple";
    let app = create_router(state);
    let response = post_json(
        app.clone(),
        "/password/verify",
        json!({"token": token, "new_password": password}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("set-cookie").is_none());
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("json response");

    assert_eq!(body["status"], "verified");
    assert!(body["subject"]
        .as_str()
        .expect("subject")
        .starts_with("user_"));
    assert_eq!(
        body,
        json!({"status": "verified", "subject": body["subject"]})
    );
    let user = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.subject.as_deref(), body["subject"].as_str());
    assert!(user.verified);
    assert!(user.registration_id.is_none());
    assert!(verify_password(
        password,
        user.password_hash.as_deref().unwrap()
    ));
    let key = StoreKey::password_verification(&verification_digest);
    assert!(storage.get(&[key.pk(), key.sk()]).await.unwrap().is_none());
    for prefix in ["oauth:code", "oauth:session", "oauth:refresh"] {
        assert!(storage.query_prefix(&[prefix]).await.unwrap().is_empty());
    }

    let response = post_json(
        app,
        "/password/verify",
        json!({"token": token, "new_password": "different password after completion"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["error"], "invalid_grant");
    let after = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.password_hash, user.password_hash);
}

#[tokio::test]
async fn password_register_rejects_legacy_and_unknown_fields_before_creating_state() {
    let (mut state, storage) = app_state_with_storage();
    let mailbox = Arc::new(Mailbox::default());
    state.email_sender = mailbox.clone();
    let app = create_router(state);

    for payload in [
        json!({"email": "user@example.com", "password": "anonymous chosen password"}),
        json!({"email": "user@example.com", "new_password": "anonymous chosen password"}),
        json!({"email": "user@example.com", "verified": true}),
    ] {
        let response = post_json(app.clone(), "/password/register", payload).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("anonymous chosen password"));
        assert!(storage.query_prefix(&[]).await.unwrap().is_empty());
        assert_eq!(mailbox.len(), 0);
    }
}

#[tokio::test]
async fn password_verify_rejects_invalid_payloads_without_consuming_a_usable_proof() {
    let (state, storage) = app_state_with_storage();
    let token = pending_token(&state, "user@example.com").await;
    let password = "owner chosen correct horse battery staple";
    let before = storage.query_prefix(&["password:"]).await.unwrap();
    let app = create_router(state);

    for payload in [
        json!({"token": token}),
        json!({"new_password": password}),
        json!({"token": token, "new_password": password, "password": "legacy secret"}),
        json!({"token": token, "new_password": password, "verified": true}),
    ] {
        let response = post_json(app.clone(), "/password/verify", payload).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let body = String::from_utf8_lossy(&body);
        assert!(!body.contains(&token));
        assert!(!body.contains(password));
        assert_eq!(storage.query_prefix(&["password:"]).await.unwrap(), before);
    }

    for rejected_password in ["short".to_string(), "x".repeat(129)] {
        let response = post_json(
            app.clone(),
            "/password/verify",
            json!({"token": token, "new_password": rejected_password}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({"error": "invalid_request", "error_description": "invalid password"}),
        );
        assert_eq!(storage.query_prefix(&["password:"]).await.unwrap(), before);
        assert!(storage
            .query_prefix(&["account:"])
            .await
            .unwrap()
            .is_empty());
    }

    let response = post_json(
        app,
        "/password/verify",
        json!({"token": token, "new_password": password}),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "rejected requests preserve proof usability"
    );
}

#[tokio::test]
async fn password_verify_unusable_proofs_return_the_same_safe_invalid_grant() {
    for reason in [
        "missing",
        "expired",
        "wrong-purpose",
        "wrong-registration",
        "missing-registration",
    ] {
        let (state, storage) = app_state_with_storage();
        let token = pending_token(&state, "user@example.com").await;
        let digest = lookup_digest(
            state.runtime.lookup_secret.as_bytes(),
            LookupFamily::EmailVerification,
            &token,
        );
        let key = StoreKey::password_verification(&digest);
        let parts = [key.pk(), key.sk()];
        let mut proof = storage.get(&parts).await.unwrap().unwrap();
        match reason {
            "missing" => storage.remove(&parts).await.unwrap(),
            _ => {
                match reason {
                    "expired" => proof["expires_at"] = json!(Utc::now() - Duration::minutes(1)),
                    "wrong-purpose" => proof["purpose"] = json!("reset_password"),
                    "wrong-registration" => proof["registration_id"] = json!("another-instance"),
                    "missing-registration" => {
                        proof.as_object_mut().unwrap().remove("registration_id");
                    }
                    _ => unreachable!(),
                }
                // Retain the physical expired record to exercise logical expiry, not fake TTL cleanup.
                storage.set(&parts, proof, None).await.unwrap();
            }
        }
        let before = storage.query_prefix(&["password:"]).await.unwrap();
        let response = post_json(
            create_router(state),
            "/password/verify",
            json!({"token": token, "new_password": "owner chosen strong password"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{reason}");
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({"error": "invalid_grant", "error_description": "verification token is invalid or expired"}),
            "{reason}",
        );
        assert_eq!(storage.query_prefix(&["password:"]).await.unwrap(), before);
        for prefix in ["account:", "identity:", "oauth:code", "oauth:refresh"] {
            assert!(
                storage.query_prefix(&[prefix]).await.unwrap().is_empty(),
                "{reason}"
            );
        }
    }
}

#[tokio::test]
async fn password_forgot_route_returns_generic_success_without_tokens() {
    let (state, storage) = app_state_with_storage();
    let app = create_router(state);
    let body = r#"{"email":"unknown@example.com"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/forgot")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("json response");
    assert_eq!(body["status"], "reset_email_sent");
    assert!(body.get("code").is_none());
    assert!(body.get("access_token").is_none());
    assert!(body.get("refresh_token").is_none());
    assert!(body.get("id_token").is_none());

    let reset_records = storage
        .query_prefix(&["password:reset"])
        .await
        .expect("query_prefix resets");
    assert!(reset_records.is_empty());
}

#[tokio::test]
async fn password_reset_route_updates_password_without_tokens() {
    let (state, storage) = app_state_with_storage();
    let runtime = state.runtime.clone();
    let store = AuthStore::new(storage.clone());
    let email = "user@example.com";
    let old_password = "correct horse battery staple";
    let new_password = "new correct horse battery staple";
    let email_digest = lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
    let subject =
        register_verified_user(&store, &runtime, &Mailbox::default(), email, old_password).await;
    let reset_token = "route-reset-token";
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

    let app = create_router(state);
    let body = r#"{"token":"route-reset-token","new_password":"new correct horse battery staple"}"#;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/reset")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&bytes).expect("json response");
    assert_eq!(body["status"], "password_reset");
    assert!(body.get("code").is_none());
    assert!(body.get("access_token").is_none());
    assert!(body.get("refresh_token").is_none());
    assert!(body.get("id_token").is_none());

    let updated = store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .expect("get user")
        .expect("user");
    let updated_hash = updated.password_hash.as_deref().expect("password hash");
    assert!(verify_password(new_password, updated_hash));
    assert!(!verify_password(old_password, updated_hash));
}

#[tokio::test]
async fn password_login_route_redirects_with_authorization_code() {
    let (state, storage) = app_state_with_storage();
    let runtime = state.runtime.clone();
    let store = AuthStore::new(storage.clone());
    let email = "user@example.com";
    let password = "correct horse battery staple";
    register_verified_user(&store, &runtime, &Mailbox::default(), email, password).await;

    let raw_session = "route-login-session";
    let session_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    store
        .create_authorize_session(
            &session_digest,
            AuthorizeSessionRecord {
                client_id: "web".to_string(),
                redirect_uri: "https://app.example.com/auth/callback".to_string(),
                state: Some("abc".to_string()),
                scope: "openid email".to_string(),
                oidc_nonce: None,
                code_challenge: Some("challenge".to_string()),
                code_challenge_method: Some("S256".to_string()),
                selected_provider: Some("password".to_string()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .expect("create session");

    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "session={raw_session}&email=user%40example.com&password=correct+horse+battery+staple"
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("location header");
    assert!(location.starts_with("https://app.example.com/auth/callback?"));
    assert!(location.contains("code="));
    assert!(location.contains("state=abc"));
    assert!(!location.contains("access_token"));
    assert!(!location.contains("refresh_token"));
    assert!(!location.contains("id_token"));
}

#[tokio::test]
async fn password_register_route_is_rate_limited_without_raw_email_keys() {
    let mut config = Config::dev();
    *config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordRegister)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap() = RateLimit {
        requests: 1,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let body = r#"{"email":"user@example.com"}"#;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::PasswordRegister.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains("user@example.com"));
}

#[tokio::test]
async fn password_register_rate_limit_uses_trusted_source_not_forwarded_headers() {
    let mut config = Config::dev();
    *config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordRegister)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap() = RateLimit {
        requests: 5,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let body = r#"{"email":"user@example.com"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/register")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "198.51.100.1")
                .header("x-real-ip", "198.51.100.2")
                .extension(api_gateway_context("203.0.113.45"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::PasswordRegister.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(keys.contains("source:"));
    assert!(!keys.contains("203.0.113.45"));
    assert!(!keys.contains("198.51.100.1"));
    assert!(!keys.contains("198.51.100.2"));
    assert!(!keys.contains("user@example.com"));
}

#[tokio::test]
async fn password_verify_route_is_rate_limited_without_raw_token_keys() {
    let mut config = Config::dev();
    config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordVerify)
        .unwrap()
        .source = RateLimit {
        requests: 1,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let token = pending_token(&state, "user@example.com").await;
    let denied_token = pending_token(&state, "other@example.com").await;
    let denied_digest = lookup_digest(
        state.runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        &denied_token,
    );
    let denied_key = StoreKey::password_verification(&denied_digest);
    let denied_parts = [denied_key.pk(), denied_key.sk()];
    let before = storage.get(&denied_parts).await.unwrap();
    assert!(before.is_some());
    let password = "correct horse battery staple";

    let app = create_router(state);
    let first = post_json(
        app.clone(),
        "/password/verify",
        json!({"token": token, "new_password": password}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let denied_body = json!({"token": denied_token, "new_password": password});
    let second = post_json(app.clone(), "/password/verify", denied_body.clone()).await;
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(storage.get(&denied_parts).await.unwrap(), before);

    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::PasswordVerify.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains(&token));
    assert!(!keys.contains(&denied_token));
    assert!(!keys.contains(password));

    let retry = super::admission::post(
        app,
        "/password/verify",
        denied_body.to_string(),
        Some("203.0.113.2"),
        true,
    )
    .await;
    assert_eq!(
        retry.status(),
        StatusCode::OK,
        "source denial leaves the proof usable"
    );
}

#[tokio::test]
async fn password_login_route_is_rate_limited_without_raw_email_or_session_keys() {
    let mut config = Config::dev();
    *config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordLogin)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap() = RateLimit {
        requests: 1,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let body = "session=raw-login-session&email=user%40example.com&password=wrong";

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(first.status(), StatusCode::TOO_MANY_REQUESTS);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::PasswordLogin.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains("user@example.com"));
    assert!(!keys.contains("raw-login-session"));
    assert!(!keys.contains("wrong"));
}

#[tokio::test]
async fn password_forgot_route_is_rate_limited_without_raw_email_keys() {
    let mut config = Config::dev();
    *config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordResetRequest)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap() = RateLimit {
        requests: 1,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let body = r#"{"email":"user@example.com"}"#;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/forgot")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/forgot")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rate_limit_records = storage
        .query_prefix(&["ratelimit", Endpoint::PasswordResetRequest.as_str(), ""])
        .await
        .expect("query_prefix rate limits");
    assert!(!rate_limit_records.is_empty());
    let keys = format!("{:?}", rate_limit_records);
    assert!(!keys.contains("user@example.com"));
}

#[tokio::test]
async fn password_reset_route_is_rate_limited_without_raw_token_or_password_keys() {
    let mut config = Config::dev();
    config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordResetComplete)
        .unwrap()
        .source = RateLimit {
        requests: 1,
        window_seconds: 60,
    };
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    let body = r#"{"token":"route-reset-token","new_password":"new correct horse battery staple"}"#;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/reset")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::BAD_REQUEST);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/password/reset")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    let rows = storage
        .query_prefix(&["ratelimit", Endpoint::PasswordResetComplete.as_str(), ""])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let keys = format!("{rows:?}");
    assert!(!keys.contains("route-reset-token"));
    assert!(!keys.contains("new correct horse battery staple"));
}
