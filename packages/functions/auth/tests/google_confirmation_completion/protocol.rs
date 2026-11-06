use super::*;

#[tokio::test]
async fn continuation_preserves_original_query_and_optional_state() {
    for state in [Some("spaces + & ? #"), None] {
        let storage = Arc::new(support::TestStorage::new());
        let mut f = Fixture::new(storage, "redirect-query");
        f.provider.session.redirect_uri =
            "http://127.0.0.1:49123/oauth/callback?fixed=x%26y&repeat=1&repeat=2".into();
        f.provider.session.state = state.map(str::to_owned);
        f.create().await;
        let result = complete(
            &f,
            &f.provider.store,
            &Default::default(),
            &f.proof,
            "203.0.113.16",
        )
        .await
        .unwrap();
        let redirect = url::Url::parse(&result.redirect_uri).unwrap();
        let mut expected = url::Url::parse(&f.provider.session.redirect_uri).unwrap();
        expected.set_query(None);
        let mut target = redirect.clone();
        target.set_query(None);
        assert_eq!(target, expected);
        let query: Vec<_> = redirect.query_pairs().collect();
        assert_eq!(
            query
                .iter()
                .filter(|(k, _)| k == "repeat")
                .map(|(_, v)| v.as_ref())
                .collect::<Vec<_>>(),
            vec!["1", "2"]
        );
        assert_eq!(query.iter().find(|(k, _)| k == "fixed").unwrap().1, "x&y");
        assert_eq!(
            query
                .iter()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.as_ref()),
            state
        );
        assert_eq!(query.iter().filter(|(k, _)| k == "code").count(), 1);
        let digest = lookup_digest(
            SECRET,
            LookupFamily::AuthorizationCode,
            &code_from_redirect(&result),
        );
        let record = f
            .provider
            .store
            .get_authorization_code(&digest)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.redirect_uri, f.provider.session.redirect_uri);
    }
}

#[tokio::test]
async fn delivered_confirmation_completes_and_exchanges_bound_tokens() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use irongate::{
        config::{AppState, Config},
        crypto::hmac_lookup::{lookup_digest, LookupFamily},
        routes::create_router,
        store::keys::StoreKey,
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;
    let storage = Arc::new(support::TestStorage::new());
    let runtime = Arc::new(provider_contract::ownership::runtime());
    let mut config = Config::dev();
    config.issuer_url = Some("https://auth.example.com".into());
    let app = create_router(AppState {
        store: irongate::store::AuthStore::new(storage.clone()),
        config: Arc::new(config),
        runtime: runtime.clone(),
        audit: support::audit::emitter(irongate::config::audit::AuditLogMode::CloudWatch),
        email_sender: Arc::new(support::NoopEmailSender),
        google_client: Arc::new(irongate::providers::google::ReqwestGoogleOidcClient::new()),
        apple_client: Arc::new(irongate::providers::apple::ReqwestAppleOidcClient::new()),
    });
    for variant in ["success", "verifier", "client", "redirect", "generation"] {
        let mut f = confirmation_contract::Fixture::new(storage.clone(), variant);
        f.provider.session.redirect_uri = "https://app.example.com/auth/callback".into();
        f.provider.session.scope = "openid".into();
        let mail = confirmation_mail::Mail::default();
        let pending = irongate::providers::email_confirmation::begin_google_confirmation(
            &f.provider.store,
            &runtime,
            &Default::default(),
            &mail,
            irongate::providers::email_confirmation::GoogleConfirmationInput {
                candidate: f.candidate(),
                session: &f.provider.session,
                browser_proof: &f.browser,
                trusted_source: Some("203.0.113.16"),
            },
        )
        .await
        .unwrap();
        f.request = pending.request;
        f.proof = mail.code();
        assert_eq!(mail.len(), 1);
        let result = complete(
            &f,
            &f.store(storage.clone()),
            &Default::default(),
            &f.proof,
            "203.0.113.16",
        )
        .await
        .unwrap();
        let raw = code_from_redirect(&result);
        let digest = lookup_digest(
            provider_contract::SECRET,
            LookupFamily::AuthorizationCode,
            &raw,
        );
        let record = f
            .provider
            .store
            .get_authorization_code(&digest)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.scope, f.provider.session.scope);
        assert_eq!(record.credential_generation, 0);
        let subject = irongate::core::subjects::Subject::from_persisted(record.subject);
        if variant == "generation" {
            let key = StoreKey::account(subject.as_str());
            let mut value = f.provider.rows(std::slice::from_ref(&key)).await[0]
                .clone()
                .unwrap();
            value["credential_generation"] = json!(1);
            f.provider.replace(&key, value).await;
        }
        let form = serde_urlencoded::to_string([
            ("grant_type", "authorization_code"),
            ("code", raw.as_str()),
            (
                "client_id",
                if variant == "client" {
                    "client-a"
                } else {
                    "web"
                },
            ),
            (
                "redirect_uri",
                if variant == "redirect" {
                    "https://other.example/callback"
                } else {
                    &f.provider.session.redirect_uri
                },
            ),
            (
                "code_verifier",
                if variant == "verifier" {
                    "wrong-verifier-with-enough-characters-for-shape"
                } else {
                    "provider-contract-downstream-verifier-with-enough-entropy"
                },
            ),
        ])
        .unwrap();
        let response = app
            .clone()
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
        let status = response.status();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        if variant == "success" {
            assert_eq!(status, StatusCode::OK);
            let claims = runtime
                .signer
                .verify_access_token(
                    body["access_token"].as_str().unwrap(),
                    "https://auth.example.com",
                    &runtime.access_token_audience,
                )
                .unwrap();
            assert_eq!(claims.sub, subject.as_str());
            let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::ES256);
            validation.set_audience(&["web"]);
            validation.set_issuer(&["https://auth.example.com"]);
            let id = jsonwebtoken::decode::<Value>(
                body["id_token"].as_str().unwrap(),
                &jsonwebtoken::DecodingKey::from_ec_pem(runtime.signer.public_key_pem().as_bytes())
                    .unwrap(),
                &validation,
            )
            .unwrap();
            assert_eq!(id.claims["nonce"], json!(f.provider.session.oidc_nonce));
        } else {
            assert!(status.is_client_error(), "{variant}");
            assert!(body.get("access_token").is_none());
        }
    }
}
