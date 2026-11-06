//! Confirmation contracts run against the runner's actual storage backend.
#[path = "sign_in_confirmation/races.rs"]
pub mod races;

use crate::{
    provider_contract::{
        self, code_digest, properties, rejected, Fixture as Provider, SECRET, TTL,
    },
    transaction_gate::{PausedStorage, PendingTransaction},
};
use chrono::{DateTime, Duration, Utc};
use irongate::{
    core::subjects::Subject,
    crypto::{
        hmac_lookup::{lookup_digest, LookupFamily},
        random::generate_random_string,
    },
    error::StorageError,
    providers::verified_email::GoogleEmailCandidate,
    storage::{StorageAdapter, TransactOperation},
    store::{
        keys::StoreKey,
        sign_in_confirmation::{ConfirmationDigests, PreparedSignInConfirmation},
        AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
    },
};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct Fixture {
    pub provider: Provider,
    pub email: String,
    pub request: String,
    pub browser: String,
    pub proof: String,
}

pub async fn prepare(
    store: &AuthStore,
    request: &str,
    browser: &str,
    code: &str,
) -> Result<Option<PreparedSignInConfirmation>, StorageError> {
    Ok(store
        .load_sign_in_confirmation_attempt(request, browser, SECRET)
        .await?
        .and_then(|attempt| store.verify_sign_in_confirmation_attempt(attempt, code, SECRET)))
}

impl Fixture {
    pub fn new(storage: Arc<dyn StorageAdapter>, label: &str) -> Self {
        Self {
            provider: Provider::new(storage, IdentityProvider::Google, label),
            email: format!("{label}@example.test"),
            request: generate_random_string(32),
            browser: generate_random_string(32),
            proof: "01234567".into(),
        }
    }
    pub fn digests(&self) -> ConfirmationDigests {
        ConfirmationDigests::from_secrets(SECRET, &self.request, &self.browser, &self.proof)
            .unwrap()
    }
    pub fn key(&self) -> StoreKey {
        StoreKey::sign_in_confirmation(self.digests().request_digest())
    }
    pub fn candidate(&self) -> GoogleEmailCandidate {
        // These store fixtures start after provider validation; callbacks test the real signature boundary.
        let claims = serde_json::from_value(
            json!({"iss":"https://accounts.google.com", "sub":self.provider.external_subject,
            "aud":"client", "iat":1, "exp":2, "email":self.email, "email_verified":false}),
        )
        .unwrap();
        GoogleEmailCandidate::from_google_claims(&claims, SECRET).unwrap()
    }
    pub fn store<S: StorageAdapter + 'static>(&self, storage: S) -> AuthStore {
        let clock = self.provider.clock.clone();
        AuthStore::new(storage).with_provider_issuance_clock(move || *clock.lock().unwrap())
    }
    pub async fn create_with(
        &self,
        store: &AuthStore,
        ttl: u64,
    ) -> Result<DateTime<Utc>, StorageError> {
        store
            .create_sign_in_confirmation(
                self.candidate(),
                &self.provider.session,
                &self.digests(),
                ttl,
                DeletedIdentityReusePolicy::Immediate,
            )
            .await
    }
    pub async fn create(&self) {
        self.create_with(&self.provider.store, 600).await.unwrap();
    }
    pub async fn row(&self) -> Option<Value> {
        self.provider.rows(&[self.key()]).await.pop().unwrap()
    }
    pub async fn prepare(&self, store: &AuthStore) -> PreparedSignInConfirmation {
        prepare(store, &self.request, &self.browser, &self.proof)
            .await
            .unwrap()
            .unwrap()
    }
    pub async fn finish(&self, code: &str) -> Result<Subject, StorageError> {
        let store = self.store(self.provider.storage.clone());
        store
            .finish_sign_in_confirmation(self.prepare(&store).await, code, TTL)
            .await
    }
    pub async fn keys(&self, code: &str) -> Vec<StoreKey> {
        let row = self.row().await.unwrap();
        let subject = row["resolution"]["account"]["subject"].as_str().unwrap();
        let mut keys = self.provider.keys(subject, code);
        keys.extend(provider_contract::ownership::ownership_keys(
            subject,
            &self.email,
        ));
        keys.push(self.key());
        keys
    }
    pub async fn pause(
        &self,
        creation: bool,
        code: &str,
    ) -> (
        PendingTransaction,
        tokio::task::JoinHandle<Result<(), StorageError>>,
    ) {
        let (paused, mut gate) = PausedStorage::new(self.provider.storage.clone());
        let store = self.store(paused);
        let task = if creation {
            let candidate = self.candidate();
            let session = self.provider.session.clone();
            let digests = self.digests();
            tokio::spawn(async move {
                store
                    .create_sign_in_confirmation(
                        candidate,
                        &session,
                        &digests,
                        600,
                        DeletedIdentityReusePolicy::Immediate,
                    )
                    .await
                    .map(|_| ())
            })
        } else {
            let prepared = self.prepare(&store).await;
            let code = code.to_string();
            tokio::spawn(async move {
                store
                    .finish_sign_in_confirmation(prepared, &code, TTL)
                    .await
                    .map(|_| ())
            })
        };
        (gate.wait().await, task)
    }
    pub async fn password_owner(&self) -> Subject {
        provider_contract::ownership::password_owner(&self.provider, &self.email).await
    }
}

pub async fn durable_issuance(storage: Arc<dyn StorageAdapter>) {
    for kind in ["fresh", "password", "apple", "google"] {
        let f = Fixture::new(storage.clone(), kind);
        let expected = match kind {
            "password" => Some(f.password_owner().await),
            "apple" | "google" => {
                let provider = if kind == "apple" {
                    IdentityProvider::Apple
                } else {
                    IdentityProvider::Google
                };
                Some(
                    Provider::new(storage.clone(), provider, &format!("owner-{kind}"))
                        .issue(properties(&f.email, "Owner"), &code_digest())
                        .await,
                )
            }
            _ => None,
        };
        f.create().await;
        let code = code_digest();
        let keys = f.keys(&code).await;
        let before = f.provider.rows(&keys).await;
        assert!(before[1..4].iter().all(Option::is_none));
        if expected.is_none() {
            assert!(before[..6].iter().all(Option::is_none));
        }
        let raw = f.row().await.unwrap().to_string();
        for secret in [&f.request, &f.browser, &f.proof] {
            assert!(!raw.contains(secret));
            assert!(!f.key().sk().contains(secret));
        }
        rejected(f.create_with(&f.provider.store, 1200).await);
        assert_eq!(f.provider.rows(&keys).await, before);
        let different_process = f.store(storage.clone());
        let prepared = f.prepare(&different_process).await;
        let replay = f.prepare(&different_process).await;
        assert_eq!(f.provider.rows(&keys).await, before);
        let subject = different_process
            .finish_sign_in_confirmation(prepared, &code, TTL)
            .await
            .unwrap();
        assert_eq!(
            subject.as_str(),
            before[6].as_ref().unwrap()["resolution"]["account"]["subject"]
                .as_str()
                .unwrap()
        );
        if let Some(owner) = expected {
            assert_eq!(owner.as_str(), subject.as_str());
        }
        f.provider
            .assert_committed(
                &subject,
                &code,
                &json!({"provider":"google","email":f.email,"email_verified":true}),
                0,
            )
            .await;
        assert!(f.row().await.is_none());
        let after = f.provider.rows(&keys).await;
        rejected(
            different_process
                .finish_sign_in_confirmation(replay, &code_digest(), TTL)
                .await,
        );
        assert_eq!(f.provider.rows(&keys).await, after);
        assert!(
            prepare(&different_process, &f.request, &f.browser, &f.proof)
                .await
                .unwrap()
                .is_none()
        );
        assert!(different_process
            .take_authorization_code(&code)
            .await
            .unwrap()
            .is_some());
        assert!(different_process
            .take_authorization_code(&code)
            .await
            .unwrap()
            .is_none());
    }
}

pub async fn proof_and_record_rejections(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "binding");
    f.create().await;
    let keys = f.keys(&code_digest()).await;
    let before = f.provider.rows(&keys).await;
    for (request, browser, code) in [
        (
            f.request.clone(),
            generate_random_string(32),
            f.proof.clone(),
        ),
        (f.request.clone(), f.browser.clone(), "76543210".into()),
        (
            generate_random_string(32),
            f.browser.clone(),
            f.proof.clone(),
        ),
    ] {
        assert!(prepare(&f.provider.store, &request, &browser, &code)
            .await
            .unwrap()
            .is_none());
        assert_eq!(f.provider.rows(&keys).await, before);
    }
    let mut other = Fixture::new(storage.clone(), "other-request");
    other.browser = f.browser.clone();
    other.create().await;
    let mut corrupt = f.row().await.unwrap();
    corrupt["email_proof_digest"] = other.row().await.unwrap()["email_proof_digest"].clone();
    f.provider.replace(&f.key(), corrupt).await;
    assert!(prepare(&f.provider.store, &f.request, &f.browser, &f.proof)
        .await
        .unwrap()
        .is_none());
    f.provider
        .replace(&f.key(), before[6].clone().unwrap())
        .await;
    let original = before[6].clone().unwrap();
    for (pointer, value) in [
        ("/purpose", json!("password_reset")),
        ("/request_digest", json!(other.digests().request_digest())),
        ("/email", json!("different@example.test")),
        ("/browser_digest", json!("")),
        ("/resolution/provider", json!("apple")),
        ("/resolution/identity_digest", json!("0".repeat(64))),
        ("/resolution/account/status", json!("disabled")),
        ("/resolution/account/subject", json!("")),
        ("/resolution/email/digest", json!("0".repeat(64))),
        ("/resolution/previous", json!("Returning")),
        ("/session/selected_provider", json!("apple")),
        ("/expires_at", original["created_at"].clone()),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        f.provider.replace(&f.key(), changed.clone()).await;
        let error = prepare(&f.provider.store, &f.request, &f.browser, &f.proof).await;
        assert!(matches!(error, Err(StorageError::DynamoDB(_))), "{pointer}");
        assert_eq!(f.row().await, Some(changed));
    }
    let mut missing = original.clone();
    missing["resolution"]["account"]
        .as_object_mut()
        .unwrap()
        .remove("credential_generation");
    f.provider.replace(&f.key(), missing).await;
    assert!(matches!(
        prepare(&f.provider.store, &f.request, &f.browser, &f.proof).await,
        Err(StorageError::DynamoDB(_))
    ));
    f.provider.replace(&f.key(), original).await;
    let (gate, task) = {
        let (paused, mut gate) = PausedStorage::before_read(storage, f.key().parts());
        let store = f.store(paused);
        let (request, browser, code) = (f.request.clone(), f.browser.clone(), f.proof.clone());
        let task = tokio::spawn(async move { prepare(&store, &request, &browser, &code).await });
        (gate.wait().await, task)
    };
    gate.fail_before_dispatch();
    assert!(matches!(
        task.await.unwrap(),
        Err(StorageError::DynamoDB(_))
    ));
}

pub async fn creation_and_expiry(storage: Arc<dyn StorageAdapter>) {
    let mut f = Fixture::new(storage.clone(), "expiry");
    for ttl in [0, u64::MAX] {
        rejected(f.create_with(&f.provider.store, ttl).await);
        assert!(f.row().await.is_none());
    }
    f.provider.session.selected_provider = Some("apple".into());
    rejected(f.create_with(&f.provider.store, 600).await);
    f.provider.session.selected_provider = Some("google".into());
    f.provider.session.expires_at = *f.provider.clock.lock().unwrap() + Duration::seconds(30);
    let expires_at = f.create_with(&f.provider.store, 600).await.unwrap();
    let row = f.row().await.unwrap();
    assert_eq!(expires_at, f.provider.session.expires_at);
    assert_eq!(row["expires_at"], json!(f.provider.session.expires_at));
    let prepared = f.prepare(&f.provider.store).await;
    *f.provider.clock.lock().unwrap() = f.provider.session.expires_at;
    assert!(prepare(&f.provider.store, &f.request, &f.browser, &f.proof)
        .await
        .unwrap()
        .is_none());
    rejected(
        f.provider
            .store
            .finish_sign_in_confirmation(prepared, &code_digest(), TTL)
            .await,
    );
    assert_eq!(f.row().await, Some(row));
    rejected(f.create_with(&f.provider.store, 600).await);
    // The confirmation's own shorter lifetime is authoritative even while the session is live.
    let short = Fixture::new(storage.clone(), "short-proof");
    short.create_with(&short.provider.store, 20).await.unwrap();
    let original = short.row().await;
    let start = *short.provider.clock.lock().unwrap();
    *short.provider.clock.lock().unwrap() = start + Duration::seconds(19);
    let prepared = short.prepare(&short.provider.store).await;
    *short.provider.clock.lock().unwrap() = start + Duration::seconds(20);
    rejected(
        short
            .provider
            .store
            .finish_sign_in_confirmation(prepared, &code_digest(), TTL)
            .await,
    );
    assert!(prepare(
        &short.provider.store,
        &short.request,
        &short.browser,
        &short.proof
    )
    .await
    .unwrap()
    .is_none());
    assert_eq!(short.row().await, original);

    // Cross the deadline on the final check, after building a completion transaction.
    *short.provider.clock.lock().unwrap() = start;
    let prepared = short.prepare(&short.provider.store).await;
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let dispatch = AuthStore::new(storage.clone()).with_provider_issuance_clock(move || {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            start
        } else {
            start + Duration::seconds(20)
        }
    });
    rejected(
        dispatch
            .finish_sign_in_confirmation(prepared, &code_digest(), TTL)
            .await,
    );
    assert_eq!(short.row().await, original);
    for ttl in [0, u64::MAX] {
        let prepared = short.prepare(&short.provider.store).await;
        assert!(matches!(
            short
                .provider
                .store
                .finish_sign_in_confirmation(prepared, &code_digest(), ttl)
                .await,
            Err(StorageError::DynamoDB(_))
        ));
        assert_eq!(short.row().await, original);
    }

    let f = Fixture::new(storage, "already-active");
    f.provider
        .issue(properties(&f.email, "Existing"), &code_digest())
        .await;
    rejected(f.create_with(&f.provider.store, 600).await);
    assert!(f.row().await.is_none());
}

pub async fn deleted_reuse(storage: Arc<dyn StorageAdapter>) {
    for blocked_owner in [false, true] {
        let f = Fixture::new(storage.clone(), &format!("deleted-{blocked_owner}"));
        let old = f
            .provider
            .issue(properties(&f.email, "Previous"), &code_digest())
            .await;
        f.provider
            .store
            .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
            .await
            .unwrap();
        *f.provider.clock.lock().unwrap() = Utc::now();
        assert!(matches!(
            f.provider
                .store
                .create_sign_in_confirmation(
                    f.candidate(),
                    &f.provider.session,
                    &f.digests(),
                    600,
                    DeletedIdentityReusePolicy::Never
                )
                .await,
            Err(StorageError::AlreadyExists(_))
        ));
        assert!(f.row().await.is_none());
        let key = if blocked_owner {
            provider_contract::ownership::ownership_keys(old.as_str(), &f.email)[0].clone()
        } else {
            f.provider.identity_key()
        };
        let original = f.provider.rows(std::slice::from_ref(&key)).await[0]
            .clone()
            .unwrap();
        let mut blocked = original.clone();
        blocked["reusable_after"] = Value::Null;
        f.provider.replace(&key, blocked).await;
        assert!(matches!(
            f.create_with(&f.provider.store, 600).await,
            Err(StorageError::AlreadyExists(_))
        ));
        assert!(f.row().await.is_none());
        f.provider.replace(&key, original).await;
        f.create().await;
        let code = code_digest();
        let subject = f.finish(&code).await.unwrap();
        assert_ne!(subject.as_str(), old.as_str());
        assert!(!f.provider.store.is_active_account(&old).await.unwrap());
        assert!(f
            .provider
            .store
            .take_authorization_code(&code)
            .await
            .unwrap()
            .is_some());
    }
}

pub async fn staged_proof(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "staged-proof");
    let deadline = f.create_with(&f.provider.store, 30).await.unwrap();
    let original = f.row().await;
    let start = *f.provider.clock.lock().unwrap();
    let load = || {
        f.provider
            .store
            .load_sign_in_confirmation_attempt(&f.request, &f.browser, SECRET)
    };
    let attempt = load().await.unwrap().unwrap();
    *f.provider.clock.lock().unwrap() = deadline;
    assert!(f
        .provider
        .store
        .verify_sign_in_confirmation_attempt(attempt, &f.proof, SECRET)
        .is_none());
    assert_eq!(f.row().await, original);
    *f.provider.clock.lock().unwrap() = start;
    for invalid in [
        "",
        "1234567",
        "012345678",
        " 01234567",
        "01234567 ",
        "１２３４５６７８",
        "76543210",
    ] {
        let attempt = load().await.unwrap().unwrap();
        assert!(f
            .provider
            .store
            .verify_sign_in_confirmation_attempt(attempt, invalid, SECRET)
            .is_none());
        assert_eq!(f.row().await, original);
    }
    // A pending replacement during admission cannot change the captured proof or be consumed.
    let attempt = load().await.unwrap().unwrap();
    let mut changed = original.clone().unwrap();
    changed["session"]["state"] = json!("changed-during-admission");
    f.provider.replace(&f.key(), changed.clone()).await;
    let prepared = f
        .provider
        .store
        .verify_sign_in_confirmation_attempt(attempt, &f.proof, SECRET)
        .unwrap();
    let code = code_digest();
    rejected(
        f.provider
            .store
            .finish_sign_in_confirmation(prepared, &code, TTL)
            .await,
    );
    assert_eq!(f.row().await, Some(changed));
    assert!(f
        .provider
        .store
        .get_authorization_code(&code)
        .await
        .unwrap()
        .is_none());
}

macro_rules! confirmation_cases {
    ($runner:path $(, $attribute:meta)?) => {
        #[tokio::test] $(#[$attribute])? async fn confirmation_deleted_reuse() { $runner(confirmation_contract::deleted_reuse).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_durable_issuance() { $runner(confirmation_contract::durable_issuance).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_proof_and_record_rejections() { $runner(confirmation_contract::proof_and_record_rejections).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_creation_and_expiry() { $runner(confirmation_contract::creation_and_expiry).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_staged_proof() { $runner(confirmation_contract::staged_proof).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_conditions() { $runner(confirmation_contract::races::conditions).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_lifecycle() { $runner(confirmation_contract::races::lifecycle).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_competing_claims() { $runner(confirmation_contract::races::competing_claims).await; }
        #[tokio::test] $(#[$attribute])? async fn confirmation_uncertain_outcomes() { $runner(confirmation_contract::races::uncertain_outcomes).await; }
    };
}
pub(crate) use confirmation_cases;
