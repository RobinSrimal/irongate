use super::*;
use irongate::store::keys::StoreKey;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

struct BrowserClient {
    token: Mutex<String>,
    exchanges: AtomicUsize,
    fetches: AtomicUsize,
}
impl BrowserClient {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            token: Mutex::new(String::new()),
            exchanges: AtomicUsize::new(0),
            fetches: AtomicUsize::new(0),
        })
    }
    fn use_nonce(&self, nonce: &str) {
        let mut claims =
            oidc_validation::claims(GOOGLE_ISSUER, GOOGLE_CLIENT_ID, nonce, Utc::now());
        claims["email"] = json!("browser@gmail.com");
        claims["email_verified"] = json!(true);
        *self.token.lock().unwrap() = oidc_validation::sign(
            &claims,
            &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap(),
            Some(TEST_KEY_ID),
        );
    }
}
#[async_trait]
impl GoogleOidcClient for BrowserClient {
    async fn exchange_code(
        &self,
        _: &irongate::config::google::GoogleConfig,
        _: GoogleCodeExchangeInput<'_>,
    ) -> Result<GoogleTokenResponse, GoogleOidcError> {
        self.exchanges.fetch_add(1, Ordering::SeqCst);
        Ok(GoogleTokenResponse {
            access_token: None,
            token_type: None,
            expires_in: None,
            id_token: self.token.lock().unwrap().clone(),
        })
    }
    async fn fetch_jwks(
        &self,
        _: &irongate::config::google::GoogleConfig,
    ) -> Result<GoogleJwks, GoogleOidcError> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(jwks())
    }
}

fn protected(response: &axum::response::Response) {
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
}
fn cleared(response: &axum::response::Response, expected: &str) {
    protected(response);
    let cookies: Vec<_> = response.headers().get_all("set-cookie").iter().collect();
    assert_eq!(cookies.len(), 1);
    let cookie = cookie::Cookie::parse(cookies[0].to_str().unwrap()).unwrap();
    assert_eq!(cookie.name(), expected);
    assert_eq!(cookie.value(), "");
    assert_eq!(cookie.max_age().unwrap().whole_seconds(), 0);
    assert_eq!(cookie.path(), Some("/"));
    assert!(cookie.domain().is_none());
    assert_eq!(cookie.secure(), Some(true));
    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.same_site(), Some(cookie::SameSite::Lax));
}
fn request(state: &str, cookie: Option<&str>, error: bool) -> Request<Body> {
    let mut builder = Request::builder().uri(format!(
        "/google/callback?state={state}&{}",
        if error {
            "error=access_denied"
        } else {
            "code=google-code"
        }
    ));
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn rejected_browser_proofs_leave_state_and_session_usable() {
    let raw = "0123456789abcdefghijklmnopqrstuv";
    let cookie = callback_cookie();
    let name = cookie.split('=').next().unwrap();
    for variant in [
        "missing",
        "wrong",
        "empty",
        "short",
        "duplicate",
        "two_headers",
        "other_transaction",
        "old_cookie",
        "query",
        "malformed",
        "unbound",
        "apple",
    ] {
        let (mut state, storage) = google_app_state_with_storage(String::new());
        let client = BrowserClient::new();
        client.use_nonce(PROVIDER_NONCE);
        state.google_client = client.clone();
        seed_google_callback_state(&storage, &state.runtime).await;
        let digest = lookup_digest(LOOKUP_SECRET, LookupFamily::ProviderState, raw);
        let key = StoreKey::provider_state(&digest);
        let original = storage.get(&[key.pk(), key.sk()]).await.unwrap().unwrap();
        if matches!(variant, "unbound" | "apple") {
            let mut value = original.clone();
            value.as_object_mut().unwrap().remove("browser_digest");
            if variant == "apple" {
                value["provider"] = json!("apple");
            }
            storage
                .set(&[key.pk(), key.sk()], value, None)
                .await
                .unwrap();
        }
        let cookies = match variant {
            "missing" | "query" => None,
            "wrong" => Some(format!("{name}={}", "z".repeat(32))),
            "empty" => Some(format!("{name}=")),
            "short" => Some(format!("{name}=short")),
            "duplicate" => Some(format!("{cookie}; {cookie}")),
            "other_transaction" => Some(format!(
                "__Host-ig-google-{}={BROWSER_PROOF}",
                "0".repeat(64)
            )),
            "old_cookie" => Some(format!("irongate_session={BROWSER_PROOF}")),
            "malformed" => Some(format!("{name}=invalid%20proof")),
            _ => Some(cookie.clone()),
        };
        let app = create_router(state);
        let before = storage.query_prefix(&["provider:state"]).await.unwrap();
        let sessions = storage.query_prefix(&["oauth:session"]).await.unwrap();
        for error in [false, true] {
            let mut req = request(raw, cookies.as_deref(), error);
            if variant == "two_headers" {
                req.headers_mut().append("cookie", cookie.parse().unwrap());
            }
            if variant == "query" {
                *req.uri_mut() = format!(
                    "/google/callback?state={raw}&code=google-code&browser={BROWSER_PROOF}"
                )
                .parse()
                .unwrap();
            }
            let response = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                response.status(),
                if variant == "unbound" {
                    StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    StatusCode::BAD_REQUEST
                },
                "{variant}"
            );
            protected(&response);
            assert!(!response.headers().contains_key("set-cookie"));
            assert_eq!(
                storage.query_prefix(&["provider:state"]).await.unwrap(),
                before
            );
            assert_eq!(
                storage.query_prefix(&["oauth:session"]).await.unwrap(),
                sessions
            );
            assert_eq!(client.exchanges.load(Ordering::SeqCst), 0);
            assert_eq!(client.fetches.load(Ordering::SeqCst), 0);
            assert!(storage
                .query_prefix(&["oauth:code"])
                .await
                .unwrap()
                .is_empty());
        }
        storage
            .set(&[key.pk(), key.sk()], original, None)
            .await
            .unwrap();
        let response = app
            .oneshot(request(
                raw,
                Some(&format!("unrelated=value; {cookie}")),
                false,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        cleared(&response, name);
        assert_eq!(client.exchanges.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn independent_starts_use_capped_cookies_and_complete_in_either_order() {
    for reverse in [false, true] {
        let (mut state, storage) = google_app_state_with_storage(String::new());
        let client = BrowserClient::new();
        state.google_client = client.clone();
        let store = state.store.clone();
        let provider_ttl = state.runtime.ttls.provider_state_seconds;
        let app = create_router(state);
        let mut transactions = Vec::new();
        for remaining in [30, provider_ttl + 60] {
            let raw_session = irongate::crypto::random::generate_random_string(32);
            let digest = lookup_digest(LOOKUP_SECRET, LookupFamily::AuthorizeSession, &raw_session);
            let mut session = callback_session();
            session.expires_at = Utc::now() + Duration::seconds(remaining as i64);
            store
                .create_authorize_session(&digest, session.clone())
                .await
                .unwrap();
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/google/authorize?session={raw_session}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            protected(&response);
            let url = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
            let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
            let cookie = cookie::Cookie::parse(
                response.headers()["set-cookie"]
                    .to_str()
                    .unwrap()
                    .to_owned(),
            )
            .unwrap();
            let state_digest =
                lookup_digest(LOOKUP_SECRET, LookupFamily::ProviderState, &query["state"]);
            assert_eq!(cookie.name(), format!("__Host-ig-google-{state_digest}"));
            assert_eq!(cookie.value().len(), 32);
            assert_eq!(cookie.path(), Some("/"));
            assert!(cookie.domain().is_none());
            assert_eq!(cookie.secure(), Some(true));
            assert_eq!(cookie.http_only(), Some(true));
            assert_eq!(cookie.same_site(), Some(cookie::SameSite::Lax));
            let age = cookie.max_age().unwrap().whole_seconds();
            assert!(age > 0 && age <= remaining.min(provider_ttl) as i64);
            let row = storage
                .get(&["provider:state", &state_digest])
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                row["browser_digest"],
                lookup_digest(LOOKUP_SECRET, LookupFamily::SignInBrowser, cookie.value())
            );
            assert!(!row.to_string().contains(cookie.value()));
            assert!(!url.as_str().contains(cookie.value()));
            let record: ProviderStateRecord = serde_json::from_value(row).unwrap();
            assert_eq!(
                record.expires_at,
                session
                    .expires_at
                    .min(record.created_at + Duration::seconds(provider_ttl as i64))
            );
            transactions.push((
                query["state"].clone(),
                cookie.to_string().split(';').next().unwrap().to_owned(),
                record.nonce,
            ));
        }
        assert_ne!(transactions[0].1, transactions[1].1);
        // The wrong transaction's cookie cannot consume either state.
        let response = app
            .clone()
            .oneshot(request(&transactions[0].0, Some(&transactions[1].1), false))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            storage
                .query_prefix(&["provider:state"])
                .await
                .unwrap()
                .len(),
            2
        );
        if reverse {
            transactions.reverse();
        }
        let jar = format!("{}; {}", transactions[0].1, transactions[1].1);
        for (index, (raw, cookie, nonce)) in transactions.iter().enumerate() {
            client.use_nonce(nonce);
            let response = app
                .clone()
                .oneshot(request(raw, Some(&jar), false))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            cleared(&response, cookie.split('=').next().unwrap());
            assert_eq!(
                storage
                    .query_prefix(&["provider:state"])
                    .await
                    .unwrap()
                    .len(),
                1 - index
            );
        }
        assert_eq!(client.exchanges.load(Ordering::SeqCst), 2);
        assert_eq!(
            storage.query_prefix(&["oauth:code"]).await.unwrap().len(),
            2
        );
    }
}

#[tokio::test]
async fn bound_decline_and_invalid_token_clear_only_the_consumed_cookie() {
    for decline in [true, false] {
        let (mut state, storage) = google_app_state_with_storage(String::new());
        let client = BrowserClient::new();
        state.google_client = client.clone();
        seed_google_callback_state(&storage, &state.runtime).await;
        let response = create_router(state)
            .oneshot(request(
                "0123456789abcdefghijklmnopqrstuv",
                Some(&callback_cookie()),
                decline,
            ))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if decline {
                StatusCode::SEE_OTHER
            } else {
                StatusCode::BAD_REQUEST
            }
        );
        cleared(&response, callback_cookie().split('=').next().unwrap());
        assert!(storage
            .query_prefix(&["oauth:session"])
            .await
            .unwrap()
            .is_empty());
        assert!(storage
            .query_prefix(&["provider:state"])
            .await
            .unwrap()
            .is_empty());
        assert!(storage
            .query_prefix(&["oauth:code"])
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            client.exchanges.load(Ordering::SeqCst),
            usize::from(!decline)
        );
        if decline {
            let url = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
            let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
            assert_eq!(params["error"], "access_denied");
            assert_eq!(params["state"], "client-state");
        }
    }
}

#[tokio::test]
async fn competing_callbacks_exchange_only_once() {
    let (mut state, storage) = google_app_state_with_storage(String::new());
    let client = BrowserClient::new();
    client.use_nonce(PROVIDER_NONCE);
    state.google_client = client.clone();
    seed_google_callback_state(&storage, &state.runtime).await;
    let (paused, mut gate) = transaction_gate::PausedStorage::when(
        storage.clone(),
        |ops| matches!(&ops[0], irongate::storage::TransactOperation::Delete { key, .. } if key[0] == "provider:state"),
    );
    state.store = AuthStore::new(paused);
    let app = create_router(state);
    let other = app.clone();
    let first = tokio::spawn(async move {
        other
            .oneshot(request(
                "0123456789abcdefghijklmnopqrstuv",
                Some(&callback_cookie()),
                false,
            ))
            .await
            .unwrap()
    });
    let pending = gate.wait().await;
    let winner = app
        .oneshot(request(
            "0123456789abcdefghijklmnopqrstuv",
            Some(&callback_cookie()),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(winner.status(), StatusCode::SEE_OTHER);
    pending.commit();
    assert_eq!(first.await.unwrap().status(), StatusCode::BAD_REQUEST);
    assert_eq!(client.exchanges.load(Ordering::SeqCst), 1);
    assert_eq!(
        storage.query_prefix(&["oauth:code"]).await.unwrap().len(),
        1
    );
}
