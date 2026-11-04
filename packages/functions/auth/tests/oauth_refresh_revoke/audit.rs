use super::*;
use irongate::config::audit::AuditLogMode;
use irongate::crypto::signing::{KmsEs256Signer, KmsPublicKey, KmsSigningOperations};
use support::audit::{assert_envelope, Capture};

async fn body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap()
}

async fn refresh_with_private_context(app: axum::Router, raw: &str) -> axum::response::Response {
    use lambda_http::aws_lambda_events::apigw::{
        ApiGatewayV2httpRequestContext, ApiGatewayV2httpRequestContextHttpDescription,
    };
    use lambda_http::request::RequestContext;
    let context = RequestContext::ApiGatewayV2(ApiGatewayV2httpRequestContext {
        http: ApiGatewayV2httpRequestContextHttpDescription {
            source_ip: Some("192.0.2.80".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    let form = serde_urlencoded::to_string([
        ("grant_type", "refresh_token"),
        ("client_id", "web"),
        ("refresh_token", raw),
        ("private_extra", "PRIVATE-BODY-MARKER"),
    ])
    .unwrap();
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/token")
            .extension(context)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", "session=PRIVATE-COOKIE-MARKER")
            .header("x-private-header", "PRIVATE-HEADER-MARKER")
            .body(Body::from(form))
            .unwrap(),
    )
    .await
    .unwrap()
}
async fn refresh_record(
    storage: &TestStorage,
    runtime: &RuntimeAuthConfig,
    raw: &str,
) -> RefreshTokenRecord {
    let digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::RefreshToken,
        raw,
    );
    let key = StoreKey::refresh_token(&digest);
    serde_json::from_value(storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap()).unwrap()
}

#[tokio::test]
async fn token_routes_emit_confirmed_events_or_explicitly_suppress_them() {
    for mode in [AuditLogMode::CloudWatch, AuditLogMode::None] {
        let (mut state, storage) = app_state_with_storage();
        let capture = Capture::default();
        state.audit = capture.emitter(mode);
        let runtime = state.runtime.clone();
        // Register, verify, and log in through the current password domain flow.
        let fixture =
            recovery_fixture::Fixture::new(Arc::new(storage.clone()), "audit-private@example.com")
                .await;
        let code = fixture
            .login(&fixture.store, recovery_fixture::OLD)
            .await
            .unwrap();
        let app = create_router(state);
        let exchange = exchange_code(
            app.clone(),
            &code,
            "test-verifier-string-with-enough-length-for-pkce",
        )
        .await;
        assert_eq!(exchange.status(), StatusCode::OK);
        let exchange = body(exchange).await;
        let raw = exchange["refresh_token"].as_str().unwrap();
        let original = refresh_record(&storage, &runtime, raw).await;
        assert_eq!(original.subject, fixture.subject);
        let rotated = refresh_with_private_context(app.clone(), raw).await;
        assert_eq!(rotated.status(), StatusCode::OK);
        let rotated = body(rotated).await;
        let next = rotated["refresh_token"].as_str().unwrap();
        let next_record = refresh_record(&storage, &runtime, next).await;
        assert_eq!(
            refresh_record(&storage, &runtime, raw).await.replaced_by,
            Some(next_record.refresh_digest.clone())
        );
        let replay = refresh_with_private_context(app.clone(), raw).await;
        assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(replay).await["error"], "invalid_grant");
        let key = StoreKey::refresh_family(&original.family_id);
        let family: RefreshTokenFamilyRecord =
            serde_json::from_value(storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap())
                .unwrap();
        assert!(family.revoked_at.is_some());
        let records = capture.records();
        if mode == AuditLogMode::CloudWatch {
            assert_eq!(records.len(), 4);
            for (record, kind) in records.iter().zip([
                "refresh_token_issued",
                "authorization_code_exchanged",
                "refresh_token_rotated",
                "refresh_token_reuse",
            ]) {
                assert_envelope(record, kind);
                assert_eq!(record["client_id"], "web");
            }
            for index in [0, 1, 2] {
                assert_eq!(records[index]["subject"], fixture.subject);
            }
            assert_eq!(records[0]["token_hash"], original.refresh_digest);
            assert_eq!(records[2]["token_hash"], next_record.refresh_digest);
            assert_eq!(records[3]["token_hash"], original.refresh_digest);
        } else {
            assert!(records.is_empty());
        }
        // Rejected code exchange must not add successful issuance/exchange events.
        assert_eq!(
            exchange_code(
                app,
                &code,
                "test-verifier-string-with-enough-length-for-pkce"
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(capture.records().len(), records.len());
        for marker in [
            code.as_str(),
            raw,
            next,
            fixture.email.as_str(),
            recovery_fixture::OLD,
            exchange["access_token"].as_str().unwrap(),
        ] {
            assert!(!capture.text().contains(marker));
        }
        assert!(storage.query_prefix(&["audit"]).await.unwrap().is_empty());
        for marker in [
            "192.0.2.80",
            "PRIVATE-BODY-MARKER",
            "PRIVATE-COOKIE-MARKER",
            "PRIVATE-HEADER-MARKER",
        ] {
            assert!(!capture.text().contains(marker));
        }
    }
}

#[tokio::test]
async fn revoke_distinguishes_confirmed_and_idempotent_outcomes_without_auditing_unknown_tokens() {
    let (mut state, storage) = app_state_with_storage();
    let capture = Capture::default();
    state.audit = capture.emitter(AuditLogMode::CloudWatch);
    let runtime = state.runtime.clone();
    let subject = seed_account_with_code(
        &state,
        "revoke-audit-code",
        "verifier",
        "openid offline_access",
    )
    .await;
    let foreign = state
        .store
        .create_refresh_token(
            state.runtime.lookup_secret.as_bytes(),
            irongate::store::refresh::CreateRefreshTokenInput {
                credential_generation: 0,
                client_id: "another-client".into(),
                subject,
                subject_type: "user".into(),
                scope: "openid offline_access".into(),
                properties: json!({}),
                expires_at: Utc::now() + Duration::days(30),
            },
        )
        .await
        .unwrap();
    let app = create_router(state);
    let exchange = exchange_code(app.clone(), "revoke-audit-code", "verifier").await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let tokens = body(exchange).await;
    let raw = tokens["refresh_token"].as_str().unwrap();
    for (index, reason) in [(2, "revoked"), (3, "already_revoked")] {
        assert_eq!(
            revoke_refresh_token(app.clone(), raw).await.status(),
            StatusCode::OK
        );
        let records = capture.records();
        assert_eq!(records.len(), index + 1);
        assert_envelope(&records[index], "refresh_token_revoked");
        assert_eq!(records[index]["reason"], reason);
        assert_eq!(records[index]["client_id"], "web");
        assert_eq!(
            records[index]["token_hash"],
            lookup_digest(
                runtime.lookup_secret.as_bytes(),
                LookupFamily::RefreshToken,
                raw
            )
        );
    }
    assert_eq!(
        revoke_refresh_token(app.clone(), "RANDOM-SECRET-TOKEN")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(capture.records().len(), 4);
    assert_eq!(
        revoke_refresh_token(app.clone(), &foreign.raw_token)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(capture.records().len(), 4);
    let foreign_key = StoreKey::refresh_family(&foreign.family_id);
    let family: RefreshTokenFamilyRecord = serde_json::from_value(
        storage
            .get(&[foreign_key.pk(), foreign_key.sk()])
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(family.revoked_at.is_none());
    assert_eq!(
        refresh_token(app, raw).await.status(),
        StatusCode::BAD_REQUEST
    );
    let record = refresh_record(&storage, &runtime, raw).await;
    let key = StoreKey::refresh_family(&record.family_id);
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap()).unwrap();
    assert!(family.revoked_at.is_some());
    assert!(!capture.text().contains(raw));
    assert!(!capture.text().contains("RANDOM-SECRET"));
    assert!(storage.query_prefix(&["audit"]).await.unwrap().is_empty());
}

#[tokio::test]
async fn failing_output_does_not_change_login_rotation_or_replay_rejection() {
    let (mut state, storage) = app_state_with_storage();
    let fallback = Capture::default();
    state.audit = support::audit::failing(&fallback);
    let runtime = state.runtime.clone();
    seed_account_with_code(
        &state,
        "failure-audit-code",
        "verifier",
        "openid offline_access",
    )
    .await;
    let app = create_router(state);
    let exchange = exchange_code(app.clone(), "failure-audit-code", "verifier").await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let tokens = body(exchange).await;
    let raw = tokens["refresh_token"].as_str().unwrap();
    assert_eq!(
        refresh_token(app.clone(), raw).await.status(),
        StatusCode::OK
    );
    assert!(refresh_record(&storage, &runtime, raw)
        .await
        .replaced_by
        .is_some());
    assert_eq!(
        refresh_token(app, raw).await.status(),
        StatusCode::BAD_REQUEST
    );
    let record = refresh_record(&storage, &runtime, raw).await;
    let key = StoreKey::refresh_family(&record.family_id);
    let family: RefreshTokenFamilyRecord =
        serde_json::from_value(storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap()).unwrap();
    assert!(family.revoked_at.is_some());
    let records = fallback.records();
    assert_eq!(records.len(), 4);
    for (record, kind) in records.iter().zip([
        "refresh_token_issued",
        "authorization_code_exchanged",
        "refresh_token_rotated",
        "refresh_token_reuse",
    ]) {
        assert_eq!(record["record_type"], "audit_emit_failed");
        assert_eq!(record["event_type"], kind);
        assert_eq!(record["category"], "output");
    }
    for marker in [raw, "SENSITIVE", "failure-audit-code", "user@example.com"] {
        assert!(!fallback.text().contains(marker));
    }
}

struct FailingSigner(Vec<u8>);
#[async_trait::async_trait]
impl KmsSigningOperations for FailingSigner {
    async fn get_public_key(&self, _: &str) -> Result<KmsPublicKey, String> {
        Ok(KmsPublicKey {
            der: self.0.clone(),
            key_spec: "ECC_NIST_P256".into(),
            key_usage: "SIGN_VERIFY".into(),
        })
    }
    async fn sign_digest(&self, _: &str, _: &[u8]) -> Result<Vec<u8>, String> {
        Err("PRIVATE-SIGNING-FAILURE".into())
    }
}

#[tokio::test]
async fn committed_rotation_is_audited_even_if_signing_the_response_fails() {
    use p256::pkcs8::{DecodePublicKey, EncodePublicKey};
    let (mut state, storage) = app_state_with_storage();
    let capture = Capture::default();
    state.audit = capture.emitter(AuditLogMode::CloudWatch);
    seed_account_with_code(
        &state,
        "signer-failure-code",
        "verifier",
        "openid offline_access",
    )
    .await;
    let exchange = exchange_code(
        create_router(state.clone()),
        "signer-failure-code",
        "verifier",
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let tokens = body(exchange).await;
    let raw = tokens["refresh_token"].as_str().unwrap();
    let der = p256::PublicKey::from_public_key_pem(state.runtime.signer.public_key_pem())
        .unwrap()
        .to_public_key_der()
        .unwrap();
    let signer = KmsEs256Signer::from_operations(
        "fail-kid".into(),
        "fail-key".into(),
        Arc::new(FailingSigner(der.as_bytes().to_vec())),
    )
    .await
    .unwrap();
    Arc::make_mut(&mut state.runtime).signer = signer.into();
    let response = refresh_token(create_router(state.clone()), raw).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let original = refresh_record(&storage, &state.runtime, raw).await;
    let next_digest = original.replaced_by.unwrap();
    let key = StoreKey::refresh_token(&next_digest);
    assert!(storage.get(&[key.pk(), key.sk()]).await.unwrap().is_some());
    let records = capture.records();
    assert_eq!(records.len(), 3);
    assert_envelope(&records[2], "refresh_token_rotated");
    assert_eq!(records[2]["token_hash"], next_digest);
    assert!(!capture.text().contains("PRIVATE-SIGNING-FAILURE"));
}
