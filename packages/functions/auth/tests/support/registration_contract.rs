//! The same registration transactions run against memory and DynamoDB Local.

use crate::{recovery_fixture, transaction_gate::PausedStorage};
use chrono::{DateTime, Duration, Utc};
use irongate::{
    config::environment::RuntimeAuthConfig,
    core::{passwords::hash_password_for_storage, subjects::Subject},
    crypto::{
        hmac_lookup::{lookup_digest, LookupFamily},
        password::verify_password,
    },
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
    store::{
        keys::StoreKey,
        records::{AccountStatus, IdentityStatus},
        registration::PreparedPasswordRegistration,
        AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
    },
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub const FIRST: &str = "first registration password";
pub const SECOND: &str = "second registration password";

pub struct Fixture {
    pub storage: Arc<dyn StorageAdapter>,
    pub store: AuthStore,
    pub runtime: RuntimeAuthConfig,
    pub email: String,
    pub email_digest: String,
    pub identity_digest: String,
    pub proof_digest: String,
    pub expires_at: DateTime<Utc>,
}

impl Fixture {
    pub async fn new(storage: Arc<dyn StorageAdapter>, email: &str) -> Self {
        let runtime = RuntimeAuthConfig::for_tests();
        let secret = runtime.lookup_secret.as_bytes();
        let email_digest = lookup_digest(secret, LookupFamily::Email, email);
        let identity_digest = lookup_digest(secret, LookupFamily::PasswordIdentity, email);
        let proof_digest = lookup_digest(
            secret,
            LookupFamily::EmailVerification,
            &uuid::Uuid::new_v4().to_string(),
        );
        let store = AuthStore::new(storage.clone());
        let expires_at = Utc::now() + Duration::hours(1);
        store
            .create_pending_password_user(&email_digest, email)
            .await
            .unwrap();
        store
            .create_email_verification(&proof_digest, &email_digest, expires_at)
            .await
            .unwrap();
        Self {
            storage,
            store,
            runtime,
            email: email.into(),
            email_digest,
            identity_digest,
            proof_digest,
            expires_at,
        }
    }

    pub async fn proof(&self) -> String {
        let digest = lookup_digest(
            self.runtime.lookup_secret.as_bytes(),
            LookupFamily::EmailVerification,
            &uuid::Uuid::new_v4().to_string(),
        );
        self.store
            .create_email_verification(&digest, &self.email_digest, self.expires_at)
            .await
            .unwrap();
        digest
    }

    pub async fn prepare(
        &self,
        store: &AuthStore,
        digest: &str,
    ) -> Option<PreparedPasswordRegistration> {
        store
            .prepare_password_registration(
                digest,
                self.runtime.lookup_secret.as_bytes(),
                DeletedIdentityReusePolicy::Immediate,
            )
            .await
            .unwrap()
    }

    pub fn keys(&self, digest: &str, subject: &str) -> Vec<StoreKey> {
        vec![
            StoreKey::password_verification(digest),
            StoreKey::password_user(&self.email_digest),
            StoreKey::account(subject),
            StoreKey::identity("password", &self.identity_digest),
            StoreKey::identity_by_subject(subject, "password", &self.identity_digest),
            StoreKey::password_user_by_subject(subject, &self.email_digest),
            StoreKey::email_owner(&self.email_digest),
            StoreKey::email_by_subject(subject, &self.email_digest),
        ]
    }

    pub async fn snapshot(&self, keys: &[StoreKey]) -> Vec<Option<Value>> {
        let mut rows = Vec::new();
        for key in keys {
            rows.push(self.storage.get(&[key.pk(), key.sk()]).await.unwrap());
        }
        rows
    }

    pub async fn active(&self, subject: &Subject, password: &str, other: &str) {
        let rows = self
            .snapshot(&self.keys(&self.proof_digest, subject.as_str()))
            .await;
        let user = rows[1].as_ref().unwrap();
        assert_eq!(user["subject"], subject.as_str());
        assert_eq!(user["email"], self.email);
        assert_eq!(user["verified"], true);
        assert!(user["registration_id"].is_null());
        assert!(!user["password_hash_updated_at"].is_null());
        let hash = user["password_hash"].as_str().unwrap();
        assert!(verify_password(password, hash));
        assert!(!verify_password(other, hash));
        let account = self.store.get_account(subject).await.unwrap().unwrap();
        assert_eq!(account.status, AccountStatus::Active);
        assert_eq!(account.credential_generation, 0);
        let identity = self
            .store
            .get_identity(IdentityProvider::Password, &self.identity_digest)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(identity.status, IdentityStatus::Active);
        assert_eq!(identity.subject.as_deref(), Some(subject.as_str()));
        assert_eq!(
            identity.properties.unwrap(),
            json!({"email": self.email, "email_verified": true})
        );
        for row in &rows[4..] {
            assert_eq!(row.as_ref().unwrap()["subject"], subject.as_str());
        }
        assert_eq!(
            rows[4].as_ref().unwrap()["identity_digest"],
            self.identity_digest
        );
        assert_eq!(rows[5].as_ref().unwrap()["email_digest"], self.email_digest);
        assert_eq!(
            rows[6],
            Some(json!({
                "email_digest": self.email_digest, "subject": subject.as_str(),
                "deleted_at": null, "reusable_after": null
            }))
        );
        assert_eq!(
            rows[7],
            Some(json!({
                "email_digest": self.email_digest, "subject": subject.as_str()
            }))
        );
    }
}

pub fn completion(operations: &[TransactOperation]) -> bool {
    operations.iter().any(
        |op| matches!(op, TransactOperation::Delete { key, .. } if key[0] == "password:verify"),
    )
}

pub fn candidate(operations: &[TransactOperation]) -> String {
    operations
        .iter()
        .find_map(|op| match op {
            TransactOperation::Put { key, value, .. } if key[0].starts_with("account:") => {
                Some(value["subject"].as_str().unwrap().to_owned())
            }
            _ => None,
        })
        .expect("completion creates an account")
}

fn assert_eight_keys(f: &Fixture, digest: &str, operations: &[TransactOperation]) -> Vec<StoreKey> {
    let keys = f.keys(digest, &candidate(operations));
    let actual = operations
        .iter()
        .map(|op| match op {
            TransactOperation::Put { key, .. }
            | TransactOperation::Delete { key, .. }
            | TransactOperation::ConditionCheck { key, .. } => key.clone(),
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(operations.len(), 8);
    assert_eq!(actual, keys.iter().map(StoreKey::parts).collect());
    keys
}

pub async fn successful_completion(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "success@example.com").await;
    let sibling = f.proof().await;
    let pending = f
        .store
        .get_password_user_by_email_digest(&f.email_digest)
        .await
        .unwrap()
        .unwrap();
    assert!(!pending.verified && pending.subject.is_none() && pending.password_hash.is_none());
    let hash = hash_password_for_storage(FIRST).unwrap();
    let (paused, mut gate) = PausedStorage::when(storage, completion);
    let store = AuthStore::new(paused);
    let prepared = f.prepare(&store, &f.proof_digest).await.unwrap();
    let task =
        tokio::spawn(async move { store.finish_password_registration(prepared, &hash).await });
    let transaction = gate.wait().await;
    let keys = assert_eight_keys(&f, &f.proof_digest, &transaction.operations);
    let before = f.snapshot(&keys).await;
    assert!(before[0].is_some() && before[1].is_some());
    assert!(before[2..].iter().all(Option::is_none));
    transaction.commit();
    let subject = task.await.unwrap().unwrap().unwrap();
    assert!(f.snapshot(&keys).await[0].is_none());
    f.active(&subject, FIRST, SECOND).await;
    assert!(f.prepare(&f.store, &f.proof_digest).await.is_none());
    assert!(f.prepare(&f.store, &sibling).await.is_none());
    assert!(f
        .storage
        .get(&["password:verify", &sibling])
        .await
        .unwrap()
        .is_some());
}

pub async fn eight_condition_rollback(storage: Arc<dyn StorageAdapter>) {
    let hash = hash_password_for_storage(FIRST).unwrap();
    for index in 0..8 {
        let f = Fixture::new(storage.clone(), &format!("rollback{index}@example.com")).await;
        let (paused, mut gate) = PausedStorage::when(storage.clone(), completion);
        let store = AuthStore::new(paused);
        let prepared = f.prepare(&store, &f.proof_digest).await.unwrap();
        let hash = hash.clone();
        let task =
            tokio::spawn(async move { store.finish_password_registration(prepared, &hash).await });
        let transaction = gate.wait().await;
        let keys = assert_eight_keys(&f, &f.proof_digest, &transaction.operations);
        let key = &keys[index];
        let mut changed = f.snapshot(&keys).await[index]
            .clone()
            .unwrap_or(json!({"occupied": true}));
        // Preserve a valid, eligible pending record; even a timestamp-only change is stale.
        if index == 0 {
            changed["created_at"] = json!(Utc::now() - Duration::minutes(1));
        }
        if index == 1 {
            changed["updated_at"] = json!(Utc::now() + Duration::seconds(1));
        }
        f.storage
            .set(
                &[key.pk(), key.sk()],
                changed,
                (index == 0).then_some(f.expires_at),
            )
            .await
            .unwrap();
        let before = f.snapshot(&keys).await;
        transaction.commit();
        assert!(task.await.unwrap().unwrap().is_none(), "condition {index}");
        assert_eq!(
            f.snapshot(&keys).await,
            before,
            "no partial writes for condition {index}"
        );
    }
}

pub async fn competing_proofs(storage: Arc<dyn StorageAdapter>) {
    let hashes = [
        hash_password_for_storage(FIRST).unwrap(),
        hash_password_for_storage(SECOND).unwrap(),
    ];
    for sibling in [false, true] {
        let f = Fixture::new(
            storage.clone(),
            &format!("competition{sibling}@example.com"),
        )
        .await;
        let second_digest = if sibling {
            f.proof().await
        } else {
            f.proof_digest.clone()
        };
        let (first_storage, mut first_gate) = PausedStorage::when(storage.clone(), completion);
        let (second_storage, mut second_gate) = PausedStorage::when(storage.clone(), completion);
        let first_store = AuthStore::new(first_storage);
        let second_store = AuthStore::new(second_storage);
        let first_prepared = f.prepare(&first_store, &f.proof_digest).await.unwrap();
        let second_prepared = f.prepare(&second_store, &second_digest).await.unwrap();
        let first_hash = hashes[0].clone();
        let second_hash = hashes[1].clone();
        let first = tokio::spawn(async move {
            first_store
                .finish_password_registration(first_prepared, &first_hash)
                .await
        });
        let first_transaction = first_gate.wait().await;
        let second = tokio::spawn(async move {
            second_store
                .finish_password_registration(second_prepared, &second_hash)
                .await
        });
        let second_transaction = second_gate.wait().await;
        // Vary which writer wins, while both have prepared the same pending instance.
        let (winner, loser, win_tx, lose_tx, password, other) = if sibling {
            (
                second,
                first,
                second_transaction,
                first_transaction,
                SECOND,
                FIRST,
            )
        } else {
            (
                first,
                second,
                first_transaction,
                second_transaction,
                FIRST,
                SECOND,
            )
        };
        let loser_subject = candidate(&lose_tx.operations);
        win_tx.commit();
        let subject = winner.await.unwrap().unwrap().unwrap();
        lose_tx.commit();
        assert!(loser.await.unwrap().unwrap().is_none());
        f.active(&subject, password, other).await;
        for key in [
            StoreKey::account(&loser_subject),
            StoreKey::identity_by_subject(&loser_subject, "password", &f.identity_digest),
            StoreKey::password_user_by_subject(&loser_subject, &f.email_digest),
            StoreKey::email_by_subject(&loser_subject, &f.email_digest),
        ] {
            assert!(
                f.snapshot(&[key]).await[0].is_none(),
                "loser has no orphan account or index"
            );
        }
        let proof_keys = [
            StoreKey::password_verification(&f.proof_digest),
            StoreKey::password_verification(&second_digest),
        ];
        let proofs = f.snapshot(&proof_keys).await;
        assert_eq!(
            proofs[0].is_some(),
            sibling,
            "only the unused sibling proof remains"
        );
        assert!(proofs[1].is_none(), "winning proof is consumed");
        assert!(f.prepare(&f.store, &f.proof_digest).await.is_none());
        assert!(f.prepare(&f.store, &second_digest).await.is_none());
        let login = recovery_fixture::Fixture {
            store: f.store.clone(),
            runtime: f.runtime.clone(),
            email: f.email.clone(),
            email_digest: f.email_digest.clone(),
            subject: subject.as_str().into(),
            mailbox: Default::default(),
        };
        assert!(matches!(
            login.login(&login.store, other).await,
            Err(irongate::providers::password::PasswordLoginError::InvalidCredentials)
        ));
        assert!(login.login(&login.store, password).await.is_ok());
    }
}

pub async fn issuance_against_completion(storage: Arc<dyn StorageAdapter>) {
    let hash = hash_password_for_storage(FIRST).unwrap();
    for issuance_wins in [false, true] {
        let f = Fixture::new(
            storage.clone(),
            &format!("issuance{issuance_wins}@example.com"),
        )
        .await;
        let digest = lookup_digest(
            f.runtime.lookup_secret.as_bytes(),
            LookupFamily::EmailVerification,
            "competing-proof",
        );
        let (paused, mut gate) = PausedStorage::new(storage.clone());
        let store = AuthStore::new(paused);
        let email_digest = f.email_digest.clone();
        let expires_at = f.expires_at;
        let issue_digest = digest.clone();
        let task = tokio::spawn(async move {
            store
                .create_email_verification(&issue_digest, &email_digest, expires_at)
                .await
        });
        let transaction = gate.wait().await;
        let prepared = f.prepare(&f.store, &f.proof_digest).await.unwrap();
        if issuance_wins {
            transaction.commit();
            task.await.unwrap().unwrap();
            f.store
                .finish_password_registration(prepared, &hash)
                .await
                .unwrap()
                .unwrap();
            assert!(f
                .storage
                .get(&["password:verify", &digest])
                .await
                .unwrap()
                .is_some());
        } else {
            f.store
                .finish_password_registration(prepared, &hash)
                .await
                .unwrap()
                .unwrap();
            transaction.commit();
            assert!(matches!(
                task.await.unwrap(),
                Err(StorageError::ConditionFailed(_))
            ));
            assert!(f
                .storage
                .get(&["password:verify", &digest])
                .await
                .unwrap()
                .is_none());
        }
        assert!(f.prepare(&f.store, &digest).await.is_none());
    }
}

pub async fn stale_identity_tombstone(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "tombstone@example.com").await;
    let old = f
        .store
        .create_account_with_identity(IdentityProvider::Password, &f.identity_digest, json!({}))
        .await
        .unwrap();
    f.store
        .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    let hash = hash_password_for_storage(FIRST).unwrap();
    let (paused, mut gate) = PausedStorage::when(storage.clone(), completion);
    let store = AuthStore::new(paused);
    let prepared = f.prepare(&store, &f.proof_digest).await.unwrap();
    let task =
        tokio::spawn(async move { store.finish_password_registration(prepared, &hash).await });
    let transaction = gate.wait().await;
    let keys = assert_eight_keys(&f, &f.proof_digest, &transaction.operations);
    let key = &keys[3];
    let mut tombstone = f.snapshot(&keys).await[3].clone().unwrap();
    tombstone["reusable_after"] = Value::Null;
    f.storage
        .set(&[key.pk(), key.sk()], tombstone, None)
        .await
        .unwrap();
    let before = f.snapshot(&keys).await;
    transaction.commit();
    assert!(task.await.unwrap().unwrap().is_none());
    assert_eq!(f.snapshot(&keys).await, before);
    assert!(f.prepare(&f.store, &f.proof_digest).await.is_none());
    assert_eq!(
        f.store.get_account(&old).await.unwrap().unwrap().status,
        AccountStatus::Deleted
    );
}

pub async fn recreated_binding(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "recreated@example.com").await;
    let sibling = f.proof().await;
    let stale = f.prepare(&f.store, &sibling).await.unwrap();
    let first = hash_password_for_storage(FIRST).unwrap();
    let second = hash_password_for_storage(SECOND).unwrap();
    let old = f
        .store
        .finish_password_registration(f.prepare(&f.store, &f.proof_digest).await.unwrap(), &first)
        .await
        .unwrap()
        .unwrap();
    f.store
        .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    f.store
        .recreate_deleted_password_user(&f.email_digest, &f.email)
        .await
        .unwrap();
    let fresh = f.proof().await;
    let proof_keys = [
        StoreKey::password_verification(&sibling),
        StoreKey::password_verification(&fresh),
        StoreKey::password_user(&f.email_digest),
    ];
    let before = f.snapshot(&proof_keys).await;
    assert_ne!(
        before[0].as_ref().unwrap()["registration_id"],
        before[1].as_ref().unwrap()["registration_id"]
    );
    let (paused, mut gate) = PausedStorage::when(storage, completion);
    let store = AuthStore::new(paused);
    let stale_hash = first.clone();
    let task =
        tokio::spawn(async move { store.finish_password_registration(stale, &stale_hash).await });
    let transaction = gate.wait().await;
    let keys = assert_eight_keys(&f, &sibling, &transaction.operations);
    let stale_before = f.snapshot(&keys).await;
    transaction.commit();
    assert!(task.await.unwrap().unwrap().is_none());
    assert_eq!(f.snapshot(&keys).await, stale_before);
    assert_eq!(f.snapshot(&proof_keys).await, before);
    assert!(f.prepare(&f.store, &sibling).await.is_none());
    let new = f
        .store
        .finish_password_registration(f.prepare(&f.store, &fresh).await.unwrap(), &second)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(old.as_str(), new.as_str());
    f.active(&new, SECOND, FIRST).await;
    assert_eq!(
        f.store.get_account(&old).await.unwrap().unwrap().status,
        AccountStatus::Deleted
    );
    for key in [
        StoreKey::identity_by_subject(old.as_str(), "password", &f.identity_digest),
        StoreKey::password_user_by_subject(old.as_str(), &f.email_digest),
        StoreKey::email_by_subject(old.as_str(), &f.email_digest),
    ] {
        assert!(f.snapshot(&[key]).await[0].is_none());
    }
}

pub async fn controlled_expiry(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage, "expiry@example.com").await;
    let now = Arc::new(Mutex::new(f.expires_at - Duration::nanoseconds(1)));
    let clock = now.clone();
    let store = f
        .store
        .clone()
        .with_registration_clock(move || *clock.lock().unwrap());
    let prepared = f.prepare(&store, &f.proof_digest).await.unwrap();
    let keys = [
        StoreKey::password_verification(&f.proof_digest),
        StoreKey::password_user(&f.email_digest),
        StoreKey::identity("password", &f.identity_digest),
        StoreKey::email_owner(&f.email_digest),
    ];
    let before = f.snapshot(&keys).await;
    *now.lock().unwrap() = f.expires_at;
    assert!(store
        .finish_password_registration(prepared, &hash_password_for_storage(FIRST).unwrap())
        .await
        .unwrap()
        .is_none());
    assert!(f.prepare(&store, &f.proof_digest).await.is_none());
    assert_eq!(
        f.snapshot(&keys).await,
        before,
        "only registration time advances; TTL has not run"
    );
    // The final check immediately before dispatch is independent of the initial finish check.
    let prepared = f.prepare(&f.store, &f.proof_digest).await.unwrap();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let expiry = f.expires_at;
    let store = f.store.clone().with_registration_clock(move || {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            expiry - Duration::nanoseconds(1)
        } else {
            expiry
        }
    });
    assert!(store
        .finish_password_registration(prepared, &hash_password_for_storage(FIRST).unwrap())
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.snapshot(&keys).await, before);
}

pub async fn uncertain_outcomes(storage: Arc<dyn StorageAdapter>) {
    let hash = hash_password_for_storage(FIRST).unwrap();
    for lost_response in [false, true] {
        let f = Fixture::new(
            storage.clone(),
            &format!("failure{lost_response}@example.com"),
        )
        .await;
        let (paused, mut gate) = PausedStorage::when(storage.clone(), completion);
        let store = AuthStore::new(paused);
        let prepared = f.prepare(&store, &f.proof_digest).await.unwrap();
        let hash = hash.clone();
        let task =
            tokio::spawn(async move { store.finish_password_registration(prepared, &hash).await });
        let transaction = gate.wait().await;
        let keys = assert_eight_keys(&f, &f.proof_digest, &transaction.operations);
        let subject = Subject::from_persisted(candidate(&transaction.operations));
        let before = f.snapshot(&keys).await;
        if lost_response {
            transaction.lose_response();
        } else {
            transaction.fail_before_dispatch();
        }
        assert!(matches!(
            task.await.unwrap(),
            Err(StorageError::DynamoDB(_))
        ));
        if lost_response {
            assert!(f.snapshot(&keys).await[0].is_none());
            f.active(&subject, FIRST, SECOND).await;
            assert!(f.prepare(&f.store, &f.proof_digest).await.is_none());
        } else {
            assert_eq!(f.snapshot(&keys).await, before);
            let prepared = f.prepare(&f.store, &f.proof_digest).await.unwrap();
            let retry = f
                .store
                .finish_password_registration(prepared, &hash_password_for_storage(SECOND).unwrap())
                .await
                .unwrap()
                .unwrap();
            f.active(&retry, SECOND, FIRST).await;
            assert!(f.store.get_account(&subject).await.unwrap().is_none());
        }
    }
}

macro_rules! registration_cases {
    ($runner:path $(, $attribute:meta)?) => {
        #[tokio::test] $(#[$attribute])? async fn registration_success() { $runner(registration_contract::successful_completion).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_eight_condition_rollback() { $runner(registration_contract::eight_condition_rollback).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_competing_proofs() { $runner(registration_contract::competing_proofs).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_issuance_race() { $runner(registration_contract::issuance_against_completion).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_stale_tombstone() { $runner(registration_contract::stale_identity_tombstone).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_recreated_binding() { $runner(registration_contract::recreated_binding).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_expiry() { $runner(registration_contract::controlled_expiry).await; }
        #[tokio::test] $(#[$attribute])? async fn registration_uncertain_outcomes() { $runner(registration_contract::uncertain_outcomes).await; }
    };
}
pub(crate) use registration_cases;
