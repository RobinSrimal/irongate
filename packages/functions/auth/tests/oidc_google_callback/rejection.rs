use super::*;
use oidc_validation::{claims, corrupt_signature, identity_state, sign};
use std::sync::atomic::{AtomicUsize, Ordering};

struct ControlledClient {
    token: String,
    initial: GoogleJwks,
    refreshed: Option<GoogleJwks>,
    fetches: AtomicUsize,
    refreshes: AtomicUsize,
}

impl ControlledClient {
    fn new(token: String, initial: GoogleJwks, refreshed: Option<GoogleJwks>) -> Arc<Self> {
        Arc::new(Self {
            token,
            initial,
            refreshed,
            fetches: AtomicUsize::new(0),
            refreshes: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl GoogleOidcClient for ControlledClient {
    async fn exchange_code(
        &self,
        _: &irongate::config::google::GoogleConfig,
        input: GoogleCodeExchangeInput<'_>,
    ) -> Result<GoogleTokenResponse, GoogleOidcError> {
        assert_eq!(input.code, "google-code");
        assert_eq!(input.code_verifier, "provider-pkce-verifier");
        assert_eq!(
            input.redirect_uri,
            "https://auth.example.com/google/callback"
        );
        Ok(GoogleTokenResponse {
            access_token: None,
            token_type: None,
            expires_in: None,

            id_token: self.token.clone(),
        })
    }

    async fn fetch_jwks(
        &self,
        _: &irongate::config::google::GoogleConfig,
    ) -> Result<GoogleJwks, GoogleOidcError> {
        assert_eq!(
            self.fetches.fetch_add(1, Ordering::SeqCst),
            0,
            "one initial lookup"
        );
        Ok(self.initial.clone())
    }

    async fn refresh_jwks(
        &self,
        _: &irongate::config::google::GoogleConfig,
    ) -> Result<GoogleJwks, GoogleOidcError> {
        assert_eq!(
            self.refreshes.fetch_add(1, Ordering::SeqCst),
            0,
            "at most one forced refresh"
        );
        self.refreshed.clone().ok_or(GoogleOidcError::JwksFetch)
    }
}

fn callback_request() -> Request<Body> {
    Request::builder()
        .uri("/google/callback?code=google-code&state=raw-provider-state")
        .body(Body::empty())
        .unwrap()
}

fn token(claims: &serde_json::Value) -> String {
    sign(
        claims,
        &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap(),
        Some(TEST_KEY_ID),
    )
}

async fn assert_rejected(client: Arc<ControlledClient>, existing: bool, refreshes: usize) {
    let (mut state, storage) = google_app_state_with_storage(String::new());
    state.google_client = client.clone();
    if existing {
        seed_identity(&state.store, "original@example.com").await;
    }
    seed_google_callback_state(&storage, &state.runtime).await;
    let before = identity_state(&storage).await;
    let response = create_router(state)
        .oneshot(callback_request())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!response.headers().contains_key(LOCATION));
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"], "invalid_grant");
    assert!(error.get("code").is_none() && error.get("access_token").is_none());
    assert!(!std::str::from_utf8(&body).unwrap().contains(&client.token));
    assert!(
        identity_state(&storage).await == before,
        "invalid callback changed identity/account/code state"
    );
    assert_eq!(client.fetches.load(Ordering::SeqCst), 1);
    assert_eq!(client.refreshes.load(Ordering::SeqCst), refreshes);
    // Provider state and authorize session are deliberately consumed before validation.
    for prefix in ["provider:state", "oauth:session"] {
        assert!(storage.query_prefix(&[prefix]).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn known_key_validation_failures_do_not_refresh_or_resolve_identity() {
    for failure in [
        "signature",
        "issuer",
        "audience",
        "nonce",
        "expiry",
        "issued_at",
        "subject",
        "use",
        "algorithm",
    ] {
        for existing in [false, true] {
            let now = Utc::now();
            let mut claims = claims(GOOGLE_ISSUER, GOOGLE_CLIENT_ID, PROVIDER_NONCE, now);
            claims["email"] = json!("changed@example.com");
            let mut keys = jwks();
            match failure {
                "issuer" => claims["iss"] = json!("https://untrusted.example"),
                "audience" => claims["aud"] = json!("other-client"),
                "nonce" => claims["nonce"] = json!("wrong-nonce"),
                "expiry" => claims["exp"] = json!(now.timestamp() - 60),
                "issued_at" => claims["iat"] = json!(now.timestamp() + 300),
                "subject" => claims["sub"] = json!(" "),
                "use" => keys.keys[0].use_ = Some("enc".into()),
                _ => {}
            }
            let token = match failure {
                "signature" => corrupt_signature(&token(&claims)),
                "algorithm" => {
                    let mut header = Header::new(Algorithm::HS256);
                    header.kid = Some(TEST_KEY_ID.into());
                    encode(
                        &header,
                        &claims,
                        &EncodingKey::from_secret(b"test-only-untrusted-key"),
                    )
                    .unwrap()
                }
                _ => token(&claims),
            };
            // A good refresh response is available, but known-key rejection must not ask for it.
            assert_rejected(
                ControlledClient::new(token, keys, Some(jwks())),
                existing,
                0,
            )
            .await;
        }
    }
}

#[tokio::test]
async fn unsuccessful_refresh_stops_without_identity_or_code_writes() {
    for failure in ["missing", "unavailable", "non_signing", "nonce"] {
        for existing in [false, true] {
            let mut claims = claims(GOOGLE_ISSUER, GOOGLE_CLIENT_ID, PROVIDER_NONCE, Utc::now());
            claims["email"] = json!("changed@example.com");
            let refreshed = match failure {
                "missing" => Some(stale_jwks()),
                "unavailable" => None,
                "non_signing" => {
                    let mut keys = jwks();
                    keys.keys[0].use_ = Some("enc".into());
                    Some(keys)
                }
                _ => {
                    claims["nonce"] = json!("wrong-nonce");
                    Some(jwks())
                }
            };
            assert_rejected(
                ControlledClient::new(token(&claims), stale_jwks(), refreshed),
                existing,
                1,
            )
            .await;
        }
    }
}

#[tokio::test]
async fn refreshed_key_issues_a_generation_bound_exchangeable_code() {
    let claims = claims(GOOGLE_ISSUER, GOOGLE_CLIENT_ID, PROVIDER_NONCE, Utc::now());
    let client = ControlledClient::new(token(&claims), stale_jwks(), Some(jwks()));
    let (mut state, storage) = google_app_state_with_storage(String::new());
    state.google_client = client.clone();
    seed_google_callback_state(&storage, &state.runtime).await;
    let app = create_router(state);
    let response = app.clone().oneshot(callback_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let url = Url::parse(response.headers().get(LOCATION).unwrap().to_str().unwrap()).unwrap();
    let code = url
        .query_pairs()
        .find(|(name, _)| name == "code")
        .unwrap()
        .1
        .into_owned();
    let codes = storage.query_prefix(&["oauth:code"]).await.unwrap();
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0].1["credential_generation"], 0);
    assert_eq!(client.fetches.load(Ordering::SeqCst), 1);
    assert_eq!(client.refreshes.load(Ordering::SeqCst), 1);
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
    let tokens: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert!(tokens["access_token"].as_str().is_some());
    assert!(tokens["id_token"].as_str().is_some());
    assert!(storage
        .query_prefix(&["oauth:code"])
        .await
        .unwrap()
        .is_empty());
}
