#[path = "support/sign_in_confirmation.rs"]
mod confirmation_contract;
#[path = "support/provider_issuance.rs"]
#[allow(dead_code, unused_imports, unused_macros)]
mod provider_contract;
mod support;
#[path = "support/transaction_gate.rs"]
mod transaction_gate;

use irongate::storage::StorageAdapter;
use std::sync::Arc;

async fn memory<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    test(Arc::new(support::TestStorage::new())).await;
}
confirmation_contract::confirmation_cases!(memory);

#[tokio::test]
async fn confirmed_codes_keep_token_exchange_bindings() {
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
        f.create().await;
        let raw = irongate::crypto::random::generate_random_string(32);
        let digest = lookup_digest(
            provider_contract::SECRET,
            LookupFamily::AuthorizationCode,
            &raw,
        );
        let subject = f.finish(&digest).await.unwrap();
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
