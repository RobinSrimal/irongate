use super::*;

pub async fn deletion_reuse(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), "deletion-reuse@example.com").await;
    let old = complete(&f, &f.proof_digest).await;
    let (paused, mut gate) = PausedStorage::when(storage, cleanup);
    let store = AuthStore::new(paused);
    let deleting = old.clone();
    let task = tokio::spawn(async move {
        store
            .delete_account(&deleting, DeletedIdentityReusePolicy::Immediate, 0)
            .await
    });
    let stale = gate.wait().await;
    // A retry finishes real cleanup while the original attempt still holds the active snapshot.
    f.store
        .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    f.store
        .recreate_deleted_password_user(&f.email_digest, &f.email)
        .await
        .unwrap();
    let proof = f.proof().await;
    let new = complete(&f, &proof).await;
    assert_ne!(new.as_str(), old.as_str());
    let keys = f.keys(&proof, new.as_str());
    let before = f.snapshot(&keys).await;
    stale.commit();
    assert!(matches!(
        task.await.unwrap(),
        Err(StorageError::ConditionFailed(_))
    ));
    assert_eq!(
        f.snapshot(&keys).await,
        before,
        "old deletion cannot rewrite a new owner"
    );
    f.store
        .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    assert_eq!(f.snapshot(&keys).await, before);
    assert!(f
        .snapshot(&[StoreKey::email_by_subject(old.as_str(), &f.email_digest)])
        .await[0]
        .is_none());
    f.active(&new, FIRST, SECOND).await;
}

pub async fn stale_index_cleanup(storage: Arc<dyn StorageAdapter>) {
    for state in ["absent", "tombstone", "new-owner"] {
        let f = Fixture::new(storage.clone(), &format!("stale-{state}@example.com")).await;
        let old = complete(&f, &f.proof_digest).await;
        let keys = owner_keys(&f, &old);
        let stale_index = f.snapshot(&keys).await[1].clone().unwrap();
        f.store
            .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
            .await
            .unwrap();
        let mut new_keys = Vec::new();
        if state == "absent" {
            f.storage
                .remove(&[keys[0].pk(), keys[0].sk()])
                .await
                .unwrap();
        } else if state == "new-owner" {
            f.store
                .recreate_deleted_password_user(&f.email_digest, &f.email)
                .await
                .unwrap();
            let proof = f.proof().await;
            let new = complete(&f, &proof).await;
            new_keys = f.keys(&proof, new.as_str());
        }
        // Restore an obsolete reverse index to exercise cleanup without ownership takeover.
        write(&f, &keys[1], stale_index.clone()).await;
        let before_owner = f.snapshot(&keys[..1]).await;
        let before_new = f.snapshot(&new_keys).await;
        f.store
            .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
            .await
            .unwrap();
        assert_eq!(f.snapshot(&keys[..1]).await, before_owner);
        assert_eq!(f.snapshot(&new_keys).await, before_new);
        assert!(f.snapshot(&keys[1..]).await[0].is_none());

        // Both the exact stale index and the owner absence/snapshot must still hold at commit.
        for changed in ["owner", "index"] {
            write(&f, &keys[1], stale_index.clone()).await;
            let (paused, mut gate) = PausedStorage::when(storage.clone(), cleanup);
            let store = AuthStore::new(paused);
            let deleting = old.clone();
            let task = tokio::spawn(async move {
                store
                    .delete_account(&deleting, DeletedIdentityReusePolicy::Immediate, 0)
                    .await
            });
            let transaction = gate.wait().await;
            let index = usize::from(changed == "index");
            let mut value = f.snapshot(&keys).await[index].clone().unwrap_or(json!({
                "email_digest": f.email_digest, "subject": "user_competing",
                "deleted_at": null, "reusable_after": null
            }));
            if index == 1 || value["subject"].is_string() {
                value["subject"] = json!("user_changed");
            } else {
                let deadline: chrono::DateTime<Utc> =
                    serde_json::from_value(value["reusable_after"].clone()).unwrap();
                value["reusable_after"] = json!(deadline + Duration::seconds(1));
            }
            write(&f, &keys[index], value).await;
            let before = f.snapshot(&keys).await;
            transaction.commit();
            assert!(matches!(
                task.await.unwrap(),
                Err(StorageError::ConditionFailed(_))
            ));
            assert_eq!(f.snapshot(&keys).await, before, "{state}/{changed}");
            // Restore only the deliberate fixture change before the next independent case.
            if let Some(owner) = &before_owner[0] {
                write(&f, &keys[0], owner.clone()).await;
            } else {
                f.storage
                    .remove(&[keys[0].pk(), keys[0].sk()])
                    .await
                    .unwrap();
            }
            write(&f, &keys[1], stale_index.clone()).await;
        }
        f.store
            .delete_account(&old, DeletedIdentityReusePolicy::Immediate, 0)
            .await
            .unwrap();
        assert_eq!(f.snapshot(&keys[..1]).await, before_owner);
        assert_eq!(f.snapshot(&new_keys).await, before_new);
        assert!(f.snapshot(&keys[1..]).await[0].is_none());
    }
}
