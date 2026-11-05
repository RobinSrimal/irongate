//! Ownership contracts share the registration fixture and real backend transactions.

#[path = "email_ownership/lifecycle.rs"]
pub mod lifecycle;
#[path = "email_ownership/races.rs"]
pub mod races;

use crate::{
    registration_contract::{self, Fixture, FIRST, SECOND},
    transaction_gate::PausedStorage,
};
use chrono::{Duration, Utc};
use irongate::{
    core::{passwords::hash_password_for_storage, subjects::Subject},
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
    store::{
        keys::StoreKey,
        records::{EmailOwnerRecord, EmailSubjectIndexRecord},
        AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
    },
};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn write(f: &Fixture, key: &StoreKey, value: Value) {
    f.storage
        .set(&[key.pk(), key.sk()], value, None)
        .await
        .unwrap();
}

pub fn owner_keys(f: &Fixture, subject: &Subject) -> [StoreKey; 2] {
    [
        StoreKey::email_owner(&f.email_digest),
        StoreKey::email_by_subject(subject.as_str(), &f.email_digest),
    ]
}

pub async fn complete(f: &Fixture, proof: &str) -> Subject {
    let prepared = f.prepare(&f.store, proof).await.unwrap();
    f.store
        .finish_password_registration(prepared, &hash_password_for_storage(FIRST).unwrap())
        .await
        .unwrap()
        .unwrap()
}

pub fn cleanup(operations: &[TransactOperation]) -> bool {
    operations.iter().any(|op| {
        matches!(op,
            TransactOperation::Delete { key, .. } if key[0].starts_with("email_by_subject:")
        )
    })
}

pub async fn active_owner_denial(storage: Arc<dyn StorageAdapter>) {
    for state in ["active", "disabled", "deleted", "missing"] {
        let f = Fixture::new(storage.clone(), &format!("owner-{state}@example.com")).await;
        // Deliberately seed an independent owner, including inconsistent account states.
        let subject = f
            .store
            .create_account_with_identity(
                IdentityProvider::Google,
                &format!("owner-{state}"),
                json!({}),
            )
            .await
            .unwrap();
        match state {
            "disabled" => {
                f.store.disable_account(&subject).await.unwrap();
            }
            "deleted" => {
                f.store
                    .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
                    .await
                    .unwrap();
            }
            "missing" => {
                let key = StoreKey::account(subject.as_str());
                f.storage.remove(&[key.pk(), key.sk()]).await.unwrap();
            }
            _ => {}
        }
        let owner = EmailOwnerRecord {
            email_digest: f.email_digest.clone(),
            subject: Some(subject.as_str().into()),
            deleted_at: None,
            reusable_after: None,
        };
        let index = EmailSubjectIndexRecord {
            email_digest: f.email_digest.clone(),
            subject: subject.as_str().into(),
        };
        let owner_keys = owner_keys(&f, &subject);
        write(&f, &owner_keys[0], serde_json::to_value(owner).unwrap()).await;
        write(&f, &owner_keys[1], serde_json::to_value(index).unwrap()).await;
        let keys = f.keys(&f.proof_digest, subject.as_str());
        let before = f.snapshot(&keys).await;
        assert!(
            f.prepare(&f.store, &f.proof_digest).await.is_none(),
            "{state}"
        );
        assert_eq!(
            f.snapshot(&keys).await,
            before,
            "{state}: no takeover or proof consumption"
        );
    }
}

pub async fn malformed_owner_and_read_failure(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "invalid-owner@example.com").await;
    let key = StoreKey::email_owner(&f.email_digest);
    let now = Utc::now();
    let valid = json!({
        "email_digest": f.email_digest, "subject": "user_existing",
        "deleted_at": null, "reusable_after": null
    });
    for (field, value) in [
        ("email_digest", json!("wrong-binding-secret")),
        ("email_digest", Value::Null),
        ("subject", Value::Null),
        ("subject", json!("  ")),
        ("subject", json!(7)),
        ("deleted_at", json!(now)),
        ("reusable_after", json!(now)),
        ("deleted_at", json!("invalid-date-secret")),
        ("unexpected", json!("unknown-field-secret")),
    ] {
        let mut malformed = valid.clone();
        malformed[field] = value;
        write(&f, &key, malformed).await;
        let keys = f.keys(&f.proof_digest, "user_existing");
        let before = f.snapshot(&keys).await;
        let result = f
            .store
            .prepare_password_registration(
                &f.proof_digest,
                f.runtime.lookup_secret.as_bytes(),
                DeletedIdentityReusePolicy::Immediate,
            )
            .await;
        let Err(StorageError::DynamoDB(message)) = result else {
            panic!("malformed owner is operational, not policy denial");
        };
        assert!(!message.contains("secret"));
        assert!(!message.contains(&f.email_digest));
        assert_eq!(f.snapshot(&keys).await, before);
    }
    write(
        &f,
        &key,
        json!({
            "email_digest": f.email_digest, "subject": null,
            "deleted_at": now, "reusable_after": now - Duration::seconds(1)
        }),
    )
    .await;
    assert!(matches!(
        f.store
            .prepare_password_registration(
                &f.proof_digest,
                f.runtime.lookup_secret.as_bytes(),
                DeletedIdentityReusePolicy::Immediate,
            )
            .await,
        Err(StorageError::DynamoDB(_))
    ));

    f.storage.remove(&[key.pk(), key.sk()]).await.unwrap();
    let before = f.snapshot(&f.keys(&f.proof_digest, "user_unused")).await;
    let (paused, mut gate) = PausedStorage::before_read(storage, key.parts());
    let store = AuthStore::new(paused);
    let proof = f.proof_digest.clone();
    let secret = f.runtime.lookup_secret.as_bytes().to_vec();
    let task = tokio::spawn(async move {
        store
            .prepare_password_registration(&proof, &secret, DeletedIdentityReusePolicy::Immediate)
            .await
    });
    gate.wait().await.fail_before_dispatch();
    assert!(matches!(
        task.await.unwrap(),
        Err(StorageError::DynamoDB(_))
    ));
    assert_eq!(
        f.snapshot(&f.keys(&f.proof_digest, "user_unused")).await,
        before
    );
    assert!(f.prepare(&f.store, &f.proof_digest).await.is_some());
}

pub async fn owner_reuse_boundaries(storage: Arc<dyn StorageAdapter>) {
    use DeletedIdentityReusePolicy::{AfterRetention, Immediate, Never};
    let hash = hash_password_for_storage(FIRST).unwrap();
    for (label, policy, permission, offset, allowed) in [
        ("never", Never, true, 0, false),
        ("immediate", Immediate, true, -1, true),
        ("missing-immediate", Immediate, false, 0, false),
        ("missing-retention", AfterRetention, false, 0, false),
        ("before", AfterRetention, true, -1, false),
        ("boundary", AfterRetention, true, 0, true),
    ] {
        let f = Fixture::new(storage.clone(), &format!("reuse-{label}@example.com")).await;
        let deadline = Utc::now() + Duration::minutes(10);
        let owner = EmailOwnerRecord {
            email_digest: f.email_digest.clone(),
            subject: None,
            deleted_at: Some(deadline - Duration::days(1)),
            reusable_after: permission.then_some(deadline),
        };
        write(
            &f,
            &StoreKey::email_owner(&f.email_digest),
            serde_json::to_value(owner).unwrap(),
        )
        .await;
        let before = f.snapshot(&f.keys(&f.proof_digest, "user_unused")).await;
        let store = f
            .store
            .clone()
            .with_registration_clock(move || deadline + Duration::nanoseconds(offset));
        let prepared = store
            .prepare_password_registration(
                &f.proof_digest,
                f.runtime.lookup_secret.as_bytes(),
                policy,
            )
            .await
            .unwrap();
        assert_eq!(prepared.is_some(), allowed, "{label}");
        assert_eq!(
            f.snapshot(&f.keys(&f.proof_digest, "user_unused")).await,
            before,
            "prepare never writes"
        );
        if let Some(prepared) = prepared {
            let subject = store
                .finish_password_registration(prepared, &hash)
                .await
                .unwrap()
                .unwrap();
            f.active(&subject, FIRST, SECOND).await;
        }
    }
    // Owner eligibility cannot override an independently forbidden password identity.
    let f = Fixture::new(storage, "identity-denies@example.com").await;
    let old = f
        .store
        .create_account_with_identity(IdentityProvider::Password, &f.identity_digest, json!({}))
        .await
        .unwrap();
    f.store.delete_account(&old, Never, 0).await.unwrap();
    let now = Utc::now();
    write(&f, &StoreKey::email_owner(&f.email_digest), json!({
        "email_digest": f.email_digest, "subject": null, "deleted_at": now, "reusable_after": now
    })).await;
    assert!(f.prepare(&f.store, &f.proof_digest).await.is_none());
}

pub async fn changed_owner_snapshot(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "changed-owner@example.com").await;
    let old = complete(&f, &f.proof_digest).await;
    f.store
        .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    f.store
        .recreate_deleted_password_user(&f.email_digest, &f.email)
        .await
        .unwrap();
    let proof = f.proof().await;
    let (paused, mut gate) = PausedStorage::when(storage, registration_contract::completion);
    let store = AuthStore::new(paused);
    let prepared = f.prepare(&store, &proof).await.unwrap();
    let hash = hash_password_for_storage(SECOND).unwrap();
    let task =
        tokio::spawn(async move { store.finish_password_registration(prepared, &hash).await });
    let transaction = gate.wait().await;
    let candidate = registration_contract::candidate(&transaction.operations);
    let keys = f.keys(&proof, &candidate);
    let mut changed = f.snapshot(&keys).await[6].clone().unwrap();
    // Keep the tombstone eligible for immediate reuse while invalidating the snapshot.
    let deadline: chrono::DateTime<Utc> =
        serde_json::from_value(changed["reusable_after"].clone()).unwrap();
    changed["reusable_after"] = json!(deadline + Duration::seconds(1));
    write(&f, &keys[6], changed).await;
    let before = f.snapshot(&keys).await;
    transaction.commit();
    assert!(task.await.unwrap().unwrap().is_none());
    assert_eq!(f.snapshot(&keys).await, before);
    let new = complete(&f, &proof).await;
    assert_ne!(new.as_str(), old.as_str());
    f.active(&new, FIRST, SECOND).await;
}

macro_rules! ownership_cases {
    ($runner:path $(, $attribute:meta)?) => {
        #[tokio::test] $(#[$attribute])? async fn ownership_active_owner_denial() { $runner(email_ownership::active_owner_denial).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_malformed_and_read_failure() { $runner(email_ownership::malformed_owner_and_read_failure).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_reuse_boundaries() { $runner(email_ownership::owner_reuse_boundaries).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_changed_snapshot() { $runner(email_ownership::changed_owner_snapshot).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_lifecycle() { $runner(email_ownership::lifecycle::lifecycle).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_cleanup_failures() { $runner(email_ownership::lifecycle::cleanup_failures).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_malformed_cleanup() { $runner(email_ownership::lifecycle::malformed_cleanup).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_deletion_reuse_race() { $runner(email_ownership::races::deletion_reuse).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_stale_index_cleanup() { $runner(email_ownership::races::stale_index_cleanup).await; }
    };
}
pub(crate) use ownership_cases;
