use super::rate_control::{Fault, RateControl};
use super::*;
use std::sync::atomic::Ordering;

pub(super) fn source_limit(config: &mut Config, endpoint: Endpoint, limit: u32) {
    config
        .rate_limit
        .policies
        .get_mut(&endpoint)
        .unwrap()
        .source
        .requests = limit;
}

pub(super) async fn post(
    app: axum::Router,
    path: &str,
    body: String,
    ip: Option<&str>,
    json: bool,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header(
            "content-type",
            if json {
                "application/json"
            } else {
                "application/x-www-form-urlencoded"
            },
        )
        .header("x-forwarded-for", uuid::Uuid::new_v4().to_string())
        .header("x-real-ip", "198.51.100.200");
    if let Some(ip) = ip {
        request = request.extension(api_gateway_context(ip));
    }
    app.oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

async fn session(state: &AppState, raw: &str, provider: &str) {
    let digest = lookup_digest(
        state.runtime.lookup_secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        raw,
    );
    state
        .store
        .create_authorize_session(
            &digest,
            AuthorizeSessionRecord {
                client_id: "web".into(),
                redirect_uri: "https://app.example.com/auth/callback".into(),
                state: None,
                scope: "openid email".into(),
                oidc_nonce: None,
                code_challenge: Some("challenge".into()),
                code_challenge_method: Some("S256".into()),
                selected_provider: Some(provider.into()),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn login_denial_precedes_password_read_and_code_issuance() {
    let mut config = Config::dev();
    config
        .rate_limit
        .policies
        .get_mut(&Endpoint::PasswordLogin)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap()
        .requests = 1;
    let (mut state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let mailbox = recovery_fixture::Mailbox::default();
    setup_password(&state, &mailbox, "user@example.com").await;
    session(&state, "login-one", "password").await;
    session(&state, "login-two", "password").await;
    let control = RateControl::new(Arc::new(storage.clone()), "source:", Fault::Observe);
    state.store = AuthStore::new(control.clone());
    let retained = state.store.clone();
    let secret = state.runtime.lookup_secret.clone();
    let app = create_router(state);
    let body = |session, email| {
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("session", session),
                ("email", email),
                ("password", recovery_fixture::OLD),
            ])
            .finish()
    };
    let first = post(
        app.clone(),
        "/password/login",
        body("login-one", "user@example.com"),
        Some("203.0.113.1"),
        false,
    )
    .await;
    assert_eq!(first.status(), StatusCode::SEE_OTHER);
    assert!(first.headers()[LOCATION]
        .to_str()
        .unwrap()
        .contains("code="));
    let reads = control.protected_reads.load(Ordering::SeqCst);
    assert!(reads > 0);
    let denied = post(
        app,
        "/password/login",
        body("login-two", " USER@EXAMPLE.COM "),
        Some("203.0.113.2"),
        false,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        control.protected_reads.load(Ordering::SeqCst),
        reads,
        "no password verification or session/code work after denial"
    );
    let digest = lookup_digest(
        secret.as_bytes(),
        LookupFamily::AuthorizeSession,
        "login-two",
    );
    assert!(retained
        .get_authorize_session(&digest)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn failures_stop_protected_routes_with_generic_unavailability() {
    for (endpoint, path, body, json) in [
        (
            Endpoint::PasswordRegister,
            "/password/register",
            r#"{"email":"user@example.com","password":"a sufficiently long password"}"#,
            true,
        ),
        (
            Endpoint::PasswordResetRequest,
            "/password/forgot",
            r#"{"email":"user@example.com"}"#,
            true,
        ),
        (
            Endpoint::PasswordLogin,
            "/password/login",
            "session=raw&email=player%40example.com&password=secret",
            false,
        ),
        (
            Endpoint::PasswordVerify,
            "/password/verify",
            r#"{"token":"raw","new_password":"a sufficiently long password"}"#,
            true,
        ),
        (
            Endpoint::PasswordResetComplete,
            "/password/reset",
            r#"{"token":"raw","new_password":"a sufficiently long password"}"#,
            true,
        ),
    ] {
        for dimension in ["source:", "email:"] {
            if dimension == "email:"
                && matches!(
                    endpoint,
                    Endpoint::PasswordVerify | Endpoint::PasswordResetComplete
                )
            {
                continue;
            }
            let (mut state, storage) = app_state_with_storage();
            let mailbox = Arc::new(recovery_fixture::Mailbox::default());
            state.email_sender = mailbox.clone();
            let control = RateControl::new(Arc::new(storage), dimension, Fault::Read);
            state.store = AuthStore::new(control.clone());
            let response = post(create_router(state), path, body.into(), None, json).await;
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{path} {dimension}"
            );
            assert!(response.headers().get("retry-after").is_none());
            assert_eq!(mailbox.len(), 0);
            assert_eq!(control.protected_reads.load(Ordering::SeqCst), 0);
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("injected"));
        }
    }
}

#[tokio::test]
async fn missing_invalid_and_equivalent_source_contexts_share_allowances() {
    for sources in [
        vec![None, Some(" "), Some("invalid")],
        vec![
            Some("2001:0DB8:0:0:0:0:0:1"),
            Some("2001:db8::1"),
            Some(" 2001:db8::1 "),
        ],
    ] {
        let mut config = Config::dev();
        source_limit(&mut config, Endpoint::PasswordRegister, 2);
        let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
        let app = create_router(state);
        for (n, source) in sources.into_iter().enumerate() {
            let response = post(
                app.clone(),
                "/password/register",
                serde_json::json!({"email": format!("person{n}@example.com"), "password": recovery_fixture::OLD}).to_string(),
                source,
                true,
            )
            .await;
            assert_eq!(
                response.status(),
                if n < 2 {
                    StatusCode::OK
                } else {
                    StatusCode::TOO_MANY_REQUESTS
                }
            );
        }
        assert_eq!(
            storage
                .query_prefix(&["ratelimit", Endpoint::PasswordRegister.as_str(), ""])
                .await
                .unwrap()
                .len(),
            3
        );
    }
}

#[tokio::test]
async fn malformed_emails_charge_source_without_account_rows() {
    let mut config = Config::dev();
    source_limit(&mut config, Endpoint::PasswordRegister, 2);
    let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let app = create_router(state);
    for (n, email) in ["no-at", "a@b", "valid@example.com"].iter().enumerate() {
        let response = post(
            app.clone(),
            "/password/register",
            serde_json::json!({"email":email, "password": recovery_fixture::OLD}).to_string(),
            None,
            true,
        )
        .await;
        assert_eq!(
            response.status(),
            if n < 2 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    assert_eq!(
        storage
            .query_prefix(&["ratelimit", Endpoint::PasswordRegister.as_str(), ""])
            .await
            .unwrap()
            .len(),
        1
    );
}

async fn oauth(
    app: axum::Router,
    endpoint: Endpoint,
    client: &str,
    ip: &str,
) -> axum::response::Response {
    if endpoint == Endpoint::Authorize {
        app.oneshot(Request::builder().uri(format!("/authorize?response_type=code&client_id={client}&redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%2Fcallback&state=test-state&scope=openid&provider=password&code_challenge=challenge&code_challenge_method=S256"))
            .header("x-forwarded-for", uuid::Uuid::new_v4().to_string())
            .extension(api_gateway_context(ip)).body(Body::empty()).unwrap()).await.unwrap()
    } else {
        let path = if endpoint == Endpoint::Token {
            "/token"
        } else {
            "/oauth/revoke"
        };
        post(
            app,
            path,
            format!("client_id={client}&grant_type=authorization_code&code=invalid&token=invalid"),
            Some(ip),
            false,
        )
        .await
    }
}

#[tokio::test]
async fn oauth_source_budget_spans_configured_and_unknown_clients() {
    for endpoint in [Endpoint::Authorize, Endpoint::Token, Endpoint::OAuthRevoke] {
        let mut config = Config::dev();
        source_limit(&mut config, endpoint, 2);
        let (mut state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
        let runtime = Arc::make_mut(&mut state.runtime);
        let web = runtime.client_registry.get("web").unwrap().clone();
        let mut second = web.clone();
        second.client_id = "web-two".into();
        runtime.client_registry = irongate::core::clients::ClientRegistry::new(vec![web, second]);
        let app = create_router(state);
        let expected = match endpoint {
            Endpoint::Authorize => StatusCode::SEE_OTHER,
            Endpoint::OAuthRevoke => StatusCode::OK,
            _ => StatusCode::BAD_REQUEST,
        };
        for client in ["web", "web-two"] {
            assert_eq!(
                oauth(app.clone(), endpoint, client, "203.0.113.1")
                    .await
                    .status(),
                expected
            );
        }
        for client in ["unknown-a", "unknown-b", "web"] {
            let response = oauth(app.clone(), endpoint, client, "203.0.113.1").await;
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            let retry_after = response.headers()["retry-after"]
                .to_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
            assert!((1..=60).contains(&retry_after));
        }
        assert_eq!(
            storage
                .query_prefix(&["ratelimit", endpoint.as_str(), ""])
                .await
                .unwrap()
                .len(),
            3,
            "no secondary rows after source exhausted"
        );
        assert_eq!(
            oauth(app, endpoint, "web", "203.0.113.2").await.status(),
            expected
        );
    }
}

#[tokio::test]
async fn unknown_clients_share_one_narrow_bucket() {
    for endpoint in [Endpoint::Authorize, Endpoint::Token, Endpoint::OAuthRevoke] {
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
        let (state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
        let app = create_router(state);
        assert_eq!(
            oauth(app.clone(), endpoint, "unknown-a", "203.0.113.1")
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            oauth(app, endpoint, "unknown-b", "203.0.113.1")
                .await
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            storage
                .query_prefix(&["ratelimit", endpoint.as_str(), ""])
                .await
                .unwrap()
                .len(),
            2
        );
    }
}

#[tokio::test]
async fn provider_sessions_and_providers_share_source_budget_before_state_creation() {
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".into());
    source_limit(&mut config, Endpoint::ProviderAuthorize, 2);
    let (mut state, storage) = app_state_with_config_and_storage(config, TestStorage::new());
    let runtime = Arc::make_mut(&mut state.runtime);
    runtime.google = irongate::config::google::GoogleConfig::from_values(
        Some("test-google"),
        Some("test-secret"),
    )
    .unwrap();
    let signer = LocalEs256Signer::generate().unwrap();
    runtime.apple = irongate::config::apple::AppleConfig::from_values(
        Some("test-apple"),
        Some("team"),
        Some("key"),
        Some("secret"),
        None,
        |_| Some(signer.signing_key().private_key_pem.clone()),
    )
    .unwrap();
    for (raw, provider) in [
        ("one", "google"),
        ("two", "apple"),
        ("three", "google"),
        ("four", "apple"),
    ] {
        session(&state, raw, provider).await;
    }
    let app = create_router(state);
    for (n, (raw, provider)) in [
        ("one", "google"),
        ("two", "apple"),
        ("three", "google"),
        ("four", "apple"),
    ]
    .into_iter()
    .enumerate()
    {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/{provider}/authorize?session={raw}"))
                    .extension(api_gateway_context("203.0.113.1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if n < 2 {
                StatusCode::SEE_OTHER
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    assert_eq!(
        storage
            .query_prefix(&["ratelimit", Endpoint::ProviderAuthorize.as_str(), ""])
            .await
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        storage
            .query_prefix(&["provider:state"])
            .await
            .unwrap()
            .len(),
        2,
        "denial creates no OAuth provider state"
    );
}

pub(super) async fn setup_password(
    state: &AppState,
    mailbox: &recovery_fixture::Mailbox,
    email: &str,
) {
    use irongate::providers::password::*;
    register_password_user(
        &state.store,
        &state.runtime,
        mailbox,
        PasswordRegistrationInput {
            email,
            password: recovery_fixture::OLD,
        },
    )
    .await
    .unwrap();
    verify_password_email(
        &state.store,
        &state.runtime,
        PasswordVerificationInput {
            token: &mailbox.last(),
        },
    )
    .await
    .unwrap();
}
