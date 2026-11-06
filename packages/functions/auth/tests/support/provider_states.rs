//! Same proof/consume contracts against memory and DynamoDB Local.
use crate::transaction_gate::PausedStorage;
use chrono::{Duration, Utc};
use irongate::{
    crypto::{
        hmac_lookup::{lookup_digest, LookupFamily},
        random::generate_random_string,
    },
    error::StorageError,
    storage::StorageAdapter,
    store::{
        keys::StoreKey,
        provider_states::ProviderStateProof,
        records::{ProviderStateBinding, ProviderStateRecord},
        AuthStore,
    },
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const SECRET: &[u8] = b"provider-state-test-secret-32-bytes";

pub struct Fixture {
    pub storage: Arc<dyn StorageAdapter>,
    pub key: StoreKey,
    pub proof: String,
    pub record: ProviderStateRecord,
}
impl Fixture {
    pub fn new(storage: Arc<dyn StorageAdapter>) -> Self {
        Self {
            storage,
            key: StoreKey::provider_state(&lookup_digest(
                SECRET,
                LookupFamily::ProviderState,
                &generate_random_string(32),
            )),
            proof: lookup_digest(
                SECRET,
                LookupFamily::SignInBrowser,
                &generate_random_string(32),
            ),
            record: ProviderStateRecord {
                session_lookup_digest: lookup_digest(
                    SECRET,
                    LookupFamily::AuthorizeSession,
                    &generate_random_string(32),
                ),
                binding: ProviderStateBinding::Apple,
                nonce: "nonce".into(),
                pkce_verifier: "verifier".into(),
                created_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        }
    }
    pub fn store(&self) -> AuthStore {
        let now = self.record.created_at;
        AuthStore::new(self.storage.clone()).with_provider_issuance_clock(move || now)
    }
    pub async fn create(&mut self) {
        self.record.binding = ProviderStateBinding::Google {
            browser_digest: self.proof.clone(),
        };
        self.store()
            .create_provider_state(self.key.sk(), self.record.clone())
            .await
            .unwrap();
    }
    pub async fn row(&self) -> Option<Value> {
        self.storage
            .get(&[self.key.pk(), self.key.sk()])
            .await
            .unwrap()
    }
    pub async fn replace(&self, value: Value) {
        self.storage
            .set(
                &[self.key.pk(), self.key.sk()],
                value,
                Some(self.record.expires_at),
            )
            .await
            .unwrap();
    }
    async fn take(
        &self,
        proof: ProviderStateProof<'_>,
    ) -> Result<Option<ProviderStateRecord>, StorageError> {
        self.store().take_provider_state(self.key.sk(), proof).await
    }
}

pub async fn proofs(storage: Arc<dyn StorageAdapter>) {
    let mut f = Fixture::new(storage.clone());
    f.create().await;
    let original = f.row().await;
    for proof in [
        ProviderStateProof::Apple,
        ProviderStateProof::GoogleBrowser(""),
        ProviderStateProof::GoogleBrowser(&"0".repeat(64)),
    ] {
        assert!(f.take(proof).await.unwrap().is_none());
        assert_eq!(f.row().await, original);
    }
    assert!(matches!(
        f.store()
            .create_provider_state(f.key.sk(), f.record.clone())
            .await,
        Err(StorageError::ConditionFailed(_))
    ));
    assert_eq!(f.row().await, original);
    let result = f
        .take(ProviderStateProof::GoogleBrowser(&f.proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.session_lookup_digest, f.record.session_lookup_digest);
    assert!(f.row().await.is_none());
    assert!(f
        .take(ProviderStateProof::GoogleBrowser(&f.proof))
        .await
        .unwrap()
        .is_none());

    let apple = Fixture::new(storage);
    apple
        .store()
        .create_provider_state(apple.key.sk(), apple.record.clone())
        .await
        .unwrap();
    let before = apple.row().await;
    assert!(apple
        .take(ProviderStateProof::GoogleBrowser(&f.proof))
        .await
        .unwrap()
        .is_none());
    assert_eq!(apple.row().await, before);
    assert!(apple
        .take(ProviderStateProof::Apple)
        .await
        .unwrap()
        .is_some());
}

pub async fn malformed_and_expiry(storage: Arc<dyn StorageAdapter>) {
    let mut f = Fixture::new(storage.clone());
    f.create().await;
    let original = f.row().await.unwrap();
    for field in [
        "browser_digest",
        "provider",
        "session_lookup_digest",
        "nonce",
        "expires_at",
    ] {
        let mut value = original.clone();
        value.as_object_mut().unwrap().remove(field);
        f.replace(value.clone()).await;
        let error = f
            .take(ProviderStateProof::GoogleBrowser(&f.proof))
            .await
            .err()
            .unwrap();
        assert!(matches!(error, StorageError::DynamoDB(_)));
        assert!(!error.to_string().contains(&f.proof));
        assert_eq!(f.row().await, Some(value));
    }
    let mut value = original.clone();
    value["browser_digest"] = json!("not-a-digest");
    f.replace(value.clone()).await;
    assert!(f
        .take(ProviderStateProof::GoogleBrowser(&f.proof))
        .await
        .is_err());
    assert_eq!(f.row().await, Some(value));
    f.replace(original.clone()).await;
    let deadline = f.record.expires_at;
    let expired = AuthStore::new(storage.clone()).with_provider_issuance_clock(move || deadline);
    assert!(expired
        .take_provider_state(f.key.sk(), ProviderStateProof::GoogleBrowser(&f.proof))
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.row().await, Some(original.clone()));
    let calls = AtomicUsize::new(0);
    let last_check = AuthStore::new(storage.clone()).with_provider_issuance_clock(move || {
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            deadline - Duration::seconds(1)
        } else {
            deadline
        }
    });
    assert!(last_check
        .take_provider_state(f.key.sk(), ProviderStateProof::GoogleBrowser(&f.proof))
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.row().await, Some(original));
    let mut new = Fixture::new(storage);
    new.record.expires_at = deadline;
    assert!(matches!(
        expired
            .create_provider_state(new.key.sk(), new.record)
            .await,
        Err(StorageError::ConditionFailed(_))
    ));
}

pub async fn races_and_failures(storage: Arc<dyn StorageAdapter>) {
    for mode in ["race", "replacement", "before", "lost", "read"] {
        let mut f = Fixture::new(storage.clone());
        f.create().await;
        let original = f.row().await;
        let (paused, mut gate) = if mode == "read" {
            PausedStorage::before_read(storage.clone(), f.key.parts())
        } else {
            PausedStorage::when(storage.clone(), |_| true)
        };
        let now = f.record.created_at;
        let store = AuthStore::new(paused).with_provider_issuance_clock(move || now);
        let digest = f.key.sk().to_owned();
        let proof = f.proof.clone();
        let job = tokio::spawn(async move {
            store
                .take_provider_state(&digest, ProviderStateProof::GoogleBrowser(&proof))
                .await
        });
        let pending = gate.wait().await;
        match mode {
            "race" => {
                assert!(f
                    .take(ProviderStateProof::GoogleBrowser(&f.proof))
                    .await
                    .unwrap()
                    .is_some());
                pending.commit();
                assert!(job.await.unwrap().unwrap().is_none());
                assert!(f.row().await.is_none());
            }
            "replacement" => {
                // Unknown fields must also be part of the exact original deletion guard.
                let mut value = original.unwrap();
                value["revision"] = json!(1);
                f.replace(value.clone()).await;
                pending.commit();
                assert!(job.await.unwrap().unwrap().is_none());
                assert_eq!(f.row().await, Some(value));
                assert!(f
                    .take(ProviderStateProof::GoogleBrowser(&f.proof))
                    .await
                    .unwrap()
                    .is_some());
            }
            "lost" => {
                pending.lose_response();
                assert!(matches!(job.await.unwrap(), Err(StorageError::DynamoDB(_))));
                assert!(f.row().await.is_none());
            }
            _ => {
                pending.fail_before_dispatch();
                assert!(matches!(job.await.unwrap(), Err(StorageError::DynamoDB(_))));
                assert_eq!(f.row().await, original);
                assert!(f
                    .take(ProviderStateProof::GoogleBrowser(&f.proof))
                    .await
                    .unwrap()
                    .is_some());
            }
        }
    }
}

macro_rules! provider_state_cases {
    ($runner:ident $(, ignore = $reason:literal)?) => {
        #[tokio::test] $(#[ignore = $reason])?
        async fn provider_state_proofs() { $runner(crate::state_contract::proofs).await; }
        #[tokio::test] $(#[ignore = $reason])?
        async fn provider_state_malformed_and_expiry() { $runner(crate::state_contract::malformed_and_expiry).await; }
        #[tokio::test] $(#[ignore = $reason])?
        async fn provider_state_races_and_failures() { $runner(crate::state_contract::races_and_failures).await; }
    };
}
pub(crate) use provider_state_cases;
