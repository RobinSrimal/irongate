use super::admission::{post, setup_password, source_limit};
use super::rate_control::{Fault, RateControl};
use super::*;
use irongate::core::clients::{ClientRegistry, ClientType, TokenEndpointAuthMethod};
use std::sync::atomic::Ordering;

fn email_request(endpoint: Endpoint, email: &str) -> (&'static str, String, bool) {
    match endpoint {
        Endpoint::PasswordRegister => (
            "/password/register",
            serde_json::json!({"email": email, "password": recovery_fixture::OLD}).to_string(),
            true,
        ),
        Endpoint::PasswordResetRequest => (
            "/password/forgot",
            serde_json::json!({"email": email}).to_string(),
            true,
        ),
        Endpoint::PasswordLogin => (
            "/password/login",
            serde_urlencoded::to_string([
                ("email", email),
                ("session", "unused-session"),
                ("password", "wrong"),
            ])
            .unwrap(),
            false,
        ),
        _ => unreachable!(),
    }
}
fn expected(endpoint: Endpoint, known: bool) -> StatusCode {
    match endpoint {
        Endpoint::PasswordLogin => StatusCode::FORBIDDEN,
        Endpoint::PasswordRegister if known => StatusCode::BAD_REQUEST,
        _ => StatusCode::OK,
    }
}

#[tokio::test]
async fn independent_email_limits_survive_source_rotation() {
    for endpoint in [
        Endpoint::PasswordRegister,
        Endpoint::PasswordLogin,
        Endpoint::PasswordResetRequest,
    ] {
        for known in [false, true] {
            let mut config = Config::dev();
            config
                .rate_limit
                .policies
                .get_mut(&endpoint)
                .unwrap()
                .secondary
                .as_mut()
                .unwrap()
                .requests = 1;
            let (mut state, storage) =
                app_state_with_config_and_storage(config, TestStorage::new());
            let mailbox = Arc::new(recovery_fixture::Mailbox::default());
            if known {
                setup_password(&state, &mailbox, "user@example.com").await;
            }
            state.email_sender = mailbox.clone();
            let control = RateControl::new(Arc::new(storage), "", Fault::Observe);
            state.store = AuthStore::new(control.clone());
            let app = create_router(state);
            let (path, body, json) = email_request(endpoint, "user@example.com");
            let first = post(app.clone(), path, body, Some("203.0.113.1"), json).await;
            assert_eq!(first.status(), expected(endpoint, known), "{endpoint:?}");
            let reads = control.protected_reads.load(Ordering::SeqCst);
            let messages = mailbox.len();
            let (_, body, _) = email_request(endpoint, " USER@EXAMPLE.COM ");
            let denied = post(app.clone(), path, body, Some("203.0.113.2"), json).await;
            assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
            assert!(
                denied.headers()["retry-after"]
                    .to_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                    > 0
            );
            assert_eq!(control.protected_reads.load(Ordering::SeqCst), reads);
            assert_eq!(mailbox.len(), messages);
            let (_, body, _) = email_request(endpoint, "other@example.com");
            assert_eq!(
                post(app, path, body, Some("203.0.113.3"), json)
                    .await
                    .status(),
                expected(endpoint, false)
            );
        }
    }
}

#[tokio::test]
async fn independent_source_limits_survive_email_rotation() {
    for endpoint in [
        Endpoint::PasswordRegister,
        Endpoint::PasswordLogin,
        Endpoint::PasswordResetRequest,
    ] {
        let mut config = Config::dev();
        source_limit(&mut config, endpoint, 2);
        let (mut state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
        let mailbox = Arc::new(recovery_fixture::Mailbox::default());
        state.email_sender = mailbox.clone();
        let control = RateControl::new(Arc::new(storage.clone()), "", Fault::Observe);
        state.store = AuthStore::new(control.clone());
        let app = create_router(state);
        for email in ["first@example.com", "second@example.com"] {
            let (path, body, json) = email_request(endpoint, email);
            assert_eq!(
                post(app.clone(), path, body, Some("203.0.113.1"), json)
                    .await
                    .status(),
                expected(endpoint, false)
            );
        }
        let reads = control.protected_reads.load(Ordering::SeqCst);
        let messages = mailbox.len();
        let (path, body, json) = email_request(endpoint, "third@example.com");
        assert_eq!(
            post(app.clone(), path, body.clone(), Some("203.0.113.1"), json)
                .await
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(control.protected_reads.load(Ordering::SeqCst), reads);
        assert_eq!(mailbox.len(), messages);
        let rows = storage
            .query_prefix(&["ratelimit", endpoint.as_str(), ""])
            .await
            .unwrap();
        assert_eq!(rows.len(), 3, "no third email counter");
        assert_eq!(
            post(app, path, body, Some("203.0.113.2"), json)
                .await
                .status(),
            expected(endpoint, false)
        );
    }
}

#[tokio::test]
async fn proof_rotation_does_not_reset_source_only_limits() {
    for (endpoint, path) in [
        (Endpoint::PasswordVerify, "/password/verify"),
        (Endpoint::PasswordResetComplete, "/password/reset"),
    ] {
        let mut config = Config::dev();
        source_limit(&mut config, endpoint, 1);
        let (mut state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
        let control = RateControl::new(Arc::new(storage), "", Fault::Observe);
        state.store = AuthStore::new(control.clone());
        let app = create_router(state);
        let body = |token| {
            serde_json::json!({"token":token,"new_password":recovery_fixture::NEW}).to_string()
        };
        assert_eq!(
            post(app.clone(), path, body("first"), Some("203.0.113.1"), true)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        let reads = control.protected_reads.load(Ordering::SeqCst);
        assert_eq!(
            post(app, path, body("second"), Some("203.0.113.1"), true)
                .await
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(control.protected_reads.load(Ordering::SeqCst), reads);
    }
}

#[tokio::test]
async fn registered_sentinel_name_does_not_share_unknown_client_budget() {
    for (endpoint, path) in [
        (Endpoint::Token, "/token"),
        (Endpoint::OAuthRevoke, "/oauth/revoke"),
    ] {
        let mut config = Config::dev();
        config
            .rate_limit
            .policies
            .get_mut(&endpoint)
            .unwrap()
            .secondary
            .as_mut()
            .unwrap()
            .requests = 1;
        let (mut state, _) = app_state_with_config_and_storage(config, TestStorage::new());
        let runtime = Arc::make_mut(&mut state.runtime);
        let mut client = runtime.client_registry.get("web").unwrap().clone();
        client.client_id = "unknown-client".into();
        runtime.client_registry = ClientRegistry::new(vec![client]);
        let app = create_router(state);
        let body =
            |id| format!("client_id={id}&grant_type=authorization_code&code=invalid&token=invalid");
        assert_eq!(
            post(
                app.clone(),
                path,
                body("unregistered"),
                Some("203.0.113.1"),
                false
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            post(
                app.clone(),
                path,
                body("another"),
                Some("203.0.113.1"),
                false
            )
            .await
            .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        let expected = if endpoint == Endpoint::Token {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::OK
        };
        assert_eq!(
            post(
                app,
                path,
                body("unknown-client"),
                Some("203.0.113.1"),
                false
            )
            .await
            .status(),
            expected
        );
    }
}

#[tokio::test]
async fn confidential_auth_failures_count_before_bearer_reads() {
    for (endpoint, path) in [
        (Endpoint::Token, "/token"),
        (Endpoint::OAuthRevoke, "/oauth/revoke"),
    ] {
        for method in [
            TokenEndpointAuthMethod::ClientSecretBasic,
            TokenEndpointAuthMethod::ClientSecretPost,
        ] {
            let mut config = Config::dev();
            config
                .rate_limit
                .policies
                .get_mut(&endpoint)
                .unwrap()
                .secondary
                .as_mut()
                .unwrap()
                .requests = 1;
            let (mut state, storage) =
                app_state_with_config_and_storage(config, TestStorage::new());
            let runtime = Arc::make_mut(&mut state.runtime);
            let mut client = runtime.client_registry.get("web").unwrap().clone();
            client.client_type = ClientType::WebConfidential;
            client.token_endpoint_auth_method = method;
            client.client_secret_hash =
                Some(hash_password_for_storage("expected client secret").unwrap());
            runtime.client_registry = ClientRegistry::new(vec![client]);
            let control = RateControl::new(Arc::new(storage), "", Fault::Observe);
            state.store = AuthStore::new(control.clone());
            let app = create_router(state);
            for status in [StatusCode::UNAUTHORIZED, StatusCode::TOO_MANY_REQUESTS] {
                let body = if method == TokenEndpointAuthMethod::ClientSecretBasic {
                    "grant_type=authorization_code&code=invalid&token=invalid"
                } else {
                    "client_id=web&client_secret=wrong&grant_type=authorization_code&code=invalid&token=invalid"
                };
                let response = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri(path)
                            .header("content-type", "application/x-www-form-urlencoded")
                            .header("authorization", "Basic d2ViOndyb25n")
                            .extension(api_gateway_context("203.0.113.1"))
                            .body(Body::from(body))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), status);
                assert_eq!(control.protected_reads.load(Ordering::SeqCst), 0);
            }
        }
    }
}

#[tokio::test]
async fn authorize_identity_follows_query_not_basic_header() {
    let mut config = Config::dev();
    config
        .rate_limit
        .policies
        .get_mut(&Endpoint::Authorize)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap()
        .requests = 1;
    let (state, _) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    for (auth, status) in [
        ("Basic YTpi", StatusCode::SEE_OTHER),
        ("Basic Yzpk", StatusCode::TOO_MANY_REQUESTS),
    ] {
        let response = app.clone().oneshot(Request::builder()
            .uri("/authorize?response_type=code&client_id=web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=test&provider=password&code_challenge=challenge&code_challenge_method=S256")
            .header("authorization", auth).extension(api_gateway_context("203.0.113.1"))
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), status);
    }
}
