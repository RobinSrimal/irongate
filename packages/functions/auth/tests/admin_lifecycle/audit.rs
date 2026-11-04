use super::*;
use support::audit::{assert_envelope, Capture};

#[tokio::test]
async fn actor_falls_back_only_within_trusted_iam_context() {
    let (mut state, _) = admin_state_with_storage();
    let (subject, _) = create_subject_with_refresh(&state.store, "web").await;
    let capture = Capture::default();
    state.audit = capture.emitter(AuditLogMode::CloudWatch);
    let app = create_admin_router(state);
    for (arn, caller, user, expected) in [
        (
            Some("arn:aws:iam::123456789012:role/operator"),
            Some("caller"),
            Some("user"),
            Some("arn:aws:iam::123456789012:role/operator"),
        ),
        (Some(""), Some("caller"), Some("user"), Some("caller")),
        (None, Some(""), Some("user"), Some("user")),
        (None, None, Some(""), None),
    ] {
        let context = ApiGatewayV2httpRequestContext {
            authorizer: Some(ApiGatewayRequestAuthorizer {
                iam: Some(ApiGatewayRequestAuthorizerIamDescription {
                    user_arn: arn.map(str::to_owned),
                    caller_id: caller.map(str::to_owned),
                    user_id: user.map(str::to_owned),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut request = admin_request("GET", format!("/_admin/users/{subject}"), false);
        request
            .extensions_mut()
            .insert(RequestContext::ApiGatewayV2(context));
        request
            .headers_mut()
            .insert("x-admin-actor", "FORGED".parse().unwrap());
        let before = capture.records().len();
        let response = app.clone().oneshot(request).await.unwrap();
        if let Some(expected) = expected {
            assert_eq!(response.status(), StatusCode::OK);
            let records = capture.records();
            assert_eq!(records.len(), before + 1);
            assert_eq!(records.last().unwrap()["actor"], expected);
        } else {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_eq!(capture.records().len(), before);
        }
    }
    assert!(!capture.text().contains("FORGED"));
}

#[tokio::test]
async fn enabled_admin_audit_reaches_output_not_database() {
    use tracing::instrument::WithSubscriber;
    for mode in [AuditLogMode::CloudWatch, AuditLogMode::None] {
        let (mut state, storage) = admin_state_with_storage_and_audit_mode(mode);
        let (subject, _) = create_subject_with_refresh(&state.store, "web").await;
        let capture = Capture::default();
        state.audit = capture.emitter(mode);
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::ERROR)
            .finish();
        let response = create_admin_router(state)
            .oneshot(admin_request(
                "GET",
                format!("/_admin/users/{subject}"),
                true,
            ))
            .with_subscriber(subscriber)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let records = capture.records();
        if mode == AuditLogMode::CloudWatch {
            assert_eq!(records.len(), 1);
            assert_envelope(&records[0], "admin_account_read");
            assert_eq!(records[0]["subject"], subject);
            assert_eq!(
                records[0]["actor"],
                "arn:aws:iam::123456789012:role/irongate-admin"
            );
        } else {
            assert!(records.is_empty());
        }
        assert!(storage.query_prefix(&["audit"]).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn lifecycle_events_have_trusted_actor_and_confirmed_structured_counts() {
    let (mut state, storage) = admin_state_with_storage();
    let capture = Capture::default();
    state.audit = capture.emitter(AuditLogMode::CloudWatch);
    let fixture = recovery_fixture::Fixture::new(
        std::sync::Arc::new(storage.clone()),
        "private-admin-audit@example.com",
    )
    .await;
    let subject = fixture.subject.clone();
    let family = state
        .store
        .create_refresh_token(
            LOOKUP_SECRET,
            CreateRefreshTokenInput {
                credential_generation: 0,
                client_id: "web".into(),
                subject: subject.clone(),
                subject_type: "user".into(),
                scope: "openid offline_access".into(),
                properties: json!({}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .unwrap()
        .family_id;
    let store = state.store.clone();
    let app = create_admin_router(state);
    for (index, action, kind, status, revoked) in [
        (
            0,
            "revoke-sessions",
            "admin_subject_sessions_revoked",
            "active",
            1,
        ),
        (1, "disable", "admin_account_disabled", "disabled", 0),
        (2, "enable", "admin_account_enabled", "active", 0),
        (3, "delete", "admin_account_deleted", "deleted", 0),
    ] {
        if action == "disable" || action == "delete" {
            fixture.request().await;
        }
        let mut request = admin_request("POST", format!("/_admin/users/{subject}/{action}"), true);
        request
            .headers_mut()
            .insert("x-admin-actor", "FORGED-ACTOR".parse().unwrap());
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
        assert_eq!(body["status"], status);
        assert_eq!(body["revoked_refresh_families"], revoked);
        let records = capture.records();
        assert_eq!(records.len(), index + 1);
        let event = &records[index];
        assert_envelope(event, kind);
        assert_eq!(event["subject"], subject);
        assert_eq!(
            event["actor"],
            "arn:aws:iam::123456789012:role/irongate-admin"
        );
        assert_eq!(event["revoked_refresh_families"], revoked);
        if action != "revoke-sessions" {
            let expected = if action == "enable" { 0 } else { 1 };
            assert_eq!(event["deleted_password_secrets"], expected);
            assert_eq!(body["deleted_password_secrets"], expected);
        }
        if action == "delete" {
            assert_eq!(event["deleted_identities"], 1);
            assert_eq!(event["deleted_password_users"], 1);
            assert_eq!(body["deleted_password_users"], 1);
        }
        let account = store
            .get_account(&Subject::from_persisted(subject.clone()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::to_value(account).unwrap()["status"], status);
    }
    let key = StoreKey::refresh_family(&family);
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap()).unwrap();
    assert!(family.revoked_at.is_some());
    // Rejected operations never masquerade as successful actions, including forged actor headers.
    for (method, suffix, iam, expected) in [
        ("GET", "", false, StatusCode::FORBIDDEN),
        ("POST", "/enable", true, StatusCode::CONFLICT),
    ] {
        let mut request = admin_request(method, format!("/_admin/users/{subject}{suffix}"), iam);
        request
            .headers_mut()
            .insert("x-admin-actor", "FORGED-ACTOR".parse().unwrap());
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            expected
        );
    }
    assert_eq!(
        app.oneshot(admin_request(
            "GET",
            "/_admin/users/user_missing".into(),
            true
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(capture.records().len(), 4);
    assert!(!capture.text().contains("FORGED"));
    assert!(!capture.text().contains("private-admin-audit@example.com"));
    assert!(storage.query_prefix(&["audit"]).await.unwrap().is_empty());
}

#[tokio::test]
async fn failing_audit_does_not_undo_admin_changes() {
    let (mut state, storage) = admin_state_with_storage();
    let fallback = Capture::default();
    state.audit = support::audit::failing(&fallback);
    let (subject, _) = create_subject_with_refresh(&state.store, "web").await;
    let store = state.store.clone();
    let response = create_admin_router(state)
        .oneshot(admin_request(
            "POST",
            format!("/_admin/users/{subject}/disable"),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!store
        .is_active_account(&Subject::from_persisted(subject))
        .await
        .unwrap());
    let records = fallback.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["record_type"], "audit_emit_failed");
    assert_eq!(records[0]["event_type"], "admin_account_disabled");
    assert_eq!(records[0]["category"], "output");
    assert!(!fallback.text().contains("SENSITIVE"));
    assert!(storage.query_prefix(&["audit"]).await.unwrap().is_empty());
}
