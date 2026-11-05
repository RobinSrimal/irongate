use super::*;
use irongate::providers::verified_email::ProviderVerifiedEmail;
use serde_json::Value;

fn claims(email: &str, hd: Option<&str>) -> Value {
    let mut claims =
        oidc_validation::claims(GOOGLE_ISSUER, GOOGLE_CLIENT_ID, PROVIDER_NONCE, Utc::now());
    claims["email"] = json!(email);
    claims["email_verified"] = json!(true);
    claims["hd"] = json!(hd);
    claims
}

fn token(claims: &Value) -> String {
    oidc_validation::sign(
        claims,
        &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap(),
        Some(TEST_KEY_ID),
    )
}

fn callback() -> Request<Body> {
    Request::builder()
        .uri("/google/callback?code=google-code&state=raw-provider-state&hd=workspace.example.com&email_verified=true")
        .body(Body::empty()).unwrap()
}

#[tokio::test]
async fn signed_authority_joins_password_or_apple_owner_and_exchanges_code() {
    for (email, hd, apple_owner) in [
        ("shared@gmail.com", None, false),
        ("shared@example.com", Some("workspace.example.com"), false),
        ("shared@example.com", Some("workspace.example.com"), true),
    ] {
        let (state, storage) = google_app_state_with_storage(token(&claims(email, hd)));
        let owner = if apple_owner {
            let apple_claims = serde_json::from_value(json!({
                "iss":"https://appleid.apple.com", "sub":"apple-owner", "aud":"apple-client",
                "iat":Utc::now().timestamp(), "exp":(Utc::now()+Duration::minutes(5)).timestamp(),
                "email":email, "email_verified":true,
            }))
            .unwrap();
            let identity = irongate::providers::apple::apple_identity_digest(
                LOOKUP_SECRET,
                "https://appleid.apple.com",
                "apple-owner",
            );
            let mut session = callback_session();
            session.selected_provider = Some("apple".into());
            let prepared = state
                .store
                .prepare_provider_issuance(
                    ProviderIdentityInput {
                        provider: IdentityProvider::Apple,
                        identity_digest: &identity,
                        properties: json!({"provider":"apple"}),
                        verified_email: ProviderVerifiedEmail::from_apple_claims(
                            &apple_claims,
                            LOOKUP_SECRET,
                        ),
                    },
                    &session,
                    300,
                    DeletedIdentityReusePolicy::Immediate,
                )
                .await
                .unwrap();
            state
                .store
                .finish_provider_issuance(prepared, "test-apple-owner-code")
                .await
                .unwrap()
        } else {
            support::registration::register_verified_user(
                &state.store,
                &state.runtime,
                &Default::default(),
                email,
                "existing account password",
            )
            .await
        };
        let digest = lookup_digest(LOOKUP_SECRET, LookupFamily::Email, email);
        let owner_before = storage.get(&["email:owner", &digest]).await.unwrap();
        seed_google_callback_state(&storage, &state.runtime).await;
        let app = create_router(state.clone());
        let response = app.clone().oneshot(callback()).await.unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let url = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.len(), 2);
        assert_eq!(query["state"], "client-state");
        let form = serde_urlencoded::to_string([
            ("grant_type", "authorization_code"),
            ("client_id", "web"),
            ("code", query["code"].as_str()),
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
        let access = state
            .runtime
            .signer
            .verify_access_token(
                tokens["access_token"].as_str().unwrap(),
                "https://auth.example.com",
                &state.runtime.access_token_audience,
            )
            .unwrap();
        assert_eq!(access.sub, owner.as_str());
        assert!(access.properties.get("hd").is_none());
        assert!(tokens["id_token"].as_str().is_some());
        assert_eq!(storage.query_prefix(&["account:"]).await.unwrap().len(), 1);
        assert_eq!(
            storage.get(&["email:owner", &digest]).await.unwrap(),
            owner_before
        );
        let identity = state
            .store
            .get_identity(
                IdentityProvider::Google,
                &google_identity_digest(LOOKUP_SECRET, GOOGLE_ISSUER, "provider-subject"),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(identity.properties.unwrap().get("hd").is_none());
    }
}

#[tokio::test]
async fn missing_authority_and_unsigned_overrides_cannot_create_or_attach() {
    let mut variants = vec![
        claims("third-party@example.com", None),
        claims("user@gmail.com.evil.test", None),
    ];
    for hd in [
        "",
        " ",
        "example..com",
        "example.com.",
        "-bad.com",
        "bad-.com",
        "https://example.com",
        "ex_ample.com",
        "éxample.com",
    ] {
        variants.push(claims("third-party@example.com", Some(hd)));
    }
    for (field, value) in [
        ("email", Value::Null),
        ("email", json!("malformed")),
        ("email_verified", Value::Null),
        ("email_verified", json!(false)),
    ] {
        let mut invalid = claims("user@gmail.com", None);
        invalid[field] = value;
        variants.push(invalid);
    }
    for owned in [false, true] {
        let (mut state, storage) = google_app_state_with_storage(String::new());
        if owned {
            support::registration::register_verified_user(
                &state.store,
                &state.runtime,
                &Default::default(),
                "third-party@example.com",
                "existing account password",
            )
            .await;
        }
        for claims in &variants {
            state.google_client = Arc::new(FakeGoogleOidcClient {
                id_token: Arc::new(token(claims)),
            });
            seed_google_callback_state(&storage, &state.runtime).await;
            let before = oidc_validation::identity_state(&storage).await;
            let response = create_router(state.clone())
                .oneshot(callback())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert!(!response.headers().contains_key(LOCATION));
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap())
                    .unwrap();
            assert_eq!(
                body,
                json!({"error":"invalid_grant","error_description":"provider sign-in is no longer valid"})
            );
            assert_eq!(oidc_validation::identity_state(&storage).await, before);
        }
        let signed = token(&claims("user@gmail.com", None));
        state.google_client = Arc::new(FakeGoogleOidcClient {
            id_token: Arc::new(oidc_validation::corrupt_signature(&signed)),
        });
        seed_google_callback_state(&storage, &state.runtime).await;
        let before = oidc_validation::identity_state(&storage).await;
        let response = create_router(state).oneshot(callback()).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(oidc_validation::identity_state(&storage).await, before);
    }
}

#[tokio::test]
async fn returning_google_identity_precedes_a_different_email_owner_without_hd() {
    let (mut state, storage) = google_app_state_with_storage(String::new());
    let original = seed_identity(&state.store, "original@example.com").await;
    let other = support::registration::register_verified_user(
        &state.store,
        &state.runtime,
        &Default::default(),
        "third-party@example.com",
        "other account password",
    )
    .await;
    assert_ne!(original.as_str(), other.as_str());
    let before = storage.query_prefix(&["email:owner"]).await.unwrap();
    for (email, verified) in [
        (json!("third-party@example.com"), json!(true)),
        (Value::Null, Value::Null),
        (json!("third-party@example.com"), json!(false)),
    ] {
        let mut signed = claims("third-party@example.com", None);
        signed["email"] = email;
        signed["email_verified"] = verified;
        state.google_client = Arc::new(FakeGoogleOidcClient {
            id_token: Arc::new(token(&signed)),
        });
        seed_google_callback_state(&storage, &state.runtime).await;
        let response = create_router(state.clone())
            .oneshot(callback())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let url = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
        let code = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .unwrap()
            .1
            .into_owned();
        let record = state
            .store
            .take_authorization_code(&lookup_digest(
                LOOKUP_SECRET,
                LookupFamily::AuthorizationCode,
                &code,
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.subject, original.as_str());
        assert_eq!(
            storage.query_prefix(&["email:owner"]).await.unwrap(),
            before
        );
    }
}
