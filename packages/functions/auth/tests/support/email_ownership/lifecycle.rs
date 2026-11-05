use super::*;
use crate::recovery_fixture;
use irongate::store::records::AccountStatus;

pub async fn lifecycle(storage: Arc<dyn StorageAdapter>) {
    use DeletedIdentityReusePolicy::{AfterRetention, Immediate, Never};
    for (label, policy, days) in [
        ("never", Never, 0),
        ("immediate", Immediate, 0),
        ("retention", AfterRetention, 3),
    ] {
        let f = Fixture::new(storage.clone(), &format!("lifecycle-{label}@example.com")).await;
        let subject = complete(&f, &f.proof_digest).await;
        let keys = owner_keys(&f, &subject);
        let active = f.snapshot(&keys).await;
        f.store.disable_account(&subject).await.unwrap();
        assert_eq!(f.snapshot(&keys).await, active);
        f.store.enable_account(&subject).await.unwrap();
        assert_eq!(f.snapshot(&keys).await, active);
        if label == "immediate" {
            let recovery = recovery_fixture::Fixture {
                store: f.store.clone(),
                runtime: f.runtime.clone(),
                email: f.email.clone(),
                email_digest: f.email_digest.clone(),
                subject: subject.as_str().into(),
                mailbox: Default::default(),
            };
            let token = recovery.request().await;
            recovery.complete(&f.store, &token).await.unwrap();
            assert!(recovery.password_is(recovery_fixture::NEW).await);
            assert_eq!(
                f.snapshot(&keys).await,
                active,
                "reset does not transfer ownership"
            );
        }
        let deleted = f
            .store
            .delete_account(&subject, policy, days)
            .await
            .unwrap();
        let deleted_at = deleted.account.deleted_at.unwrap();
        let reusable_after = match policy {
            Never => None,
            Immediate => Some(deleted_at),
            AfterRetention => Some(deleted_at + Duration::days(i64::from(days))),
        };
        let tombstone = Some(json!({
            "email_digest": f.email_digest, "subject": null,
            "deleted_at": deleted_at, "reusable_after": reusable_after,
        }));
        assert_eq!(f.snapshot(&keys).await, vec![tombstone.clone(), None]);
        assert!(f
            .storage
            .query_prefix(&[&StoreKey::email_by_subject_pk(subject.as_str())])
            .await
            .unwrap()
            .is_empty());
        let repeated = f
            .store
            .delete_account(&subject, policy, days)
            .await
            .unwrap();
        assert_eq!(repeated.account.deleted_at, Some(deleted_at));
        assert_eq!(f.snapshot(&keys).await, vec![tombstone, None]);
    }
}

pub async fn cleanup_failures(storage: Arc<dyn StorageAdapter>) {
    for failure in ["before", "lost", "owner-condition", "index-condition"] {
        let f = Fixture::new(storage.clone(), &format!("cleanup-{failure}@example.com")).await;
        let subject = complete(&f, &f.proof_digest).await;
        let keys = owner_keys(&f, &subject);
        let (paused, mut gate) = PausedStorage::when(storage.clone(), cleanup);
        let store = AuthStore::new(paused);
        let deleting = subject.clone();
        let task = tokio::spawn(async move {
            store
                .delete_account(&deleting, DeletedIdentityReusePolicy::Immediate, 0)
                .await
        });
        let transaction = gate.wait().await;
        let account = f.store.get_account(&subject).await.unwrap().unwrap();
        assert_eq!(
            account.status,
            AccountStatus::Deleted,
            "deletion is not globally atomic"
        );
        let deleted_at = account.deleted_at.unwrap();
        assert_eq!(
            f.snapshot(&keys).await[0].as_ref().unwrap()["subject"],
            subject.as_str()
        );
        assert!(
            f.snapshot(&keys).await[1].is_some(),
            "index stays until cleanup commits"
        );
        let original = f.snapshot(&keys).await;
        if failure.ends_with("condition") {
            let i = usize::from(failure == "index-condition");
            let mut value = f.snapshot(&keys).await[i].clone().unwrap();
            if i == 0 {
                // Same valid active state but a different raw value must invalidate the guard.
                value.as_object_mut().unwrap().remove("reusable_after");
            } else {
                value["subject"] = json!("user_competing");
            }
            write(&f, &keys[i], value).await;
        }
        let before = f.snapshot(&keys).await;
        match failure {
            "before" => transaction.fail_before_dispatch(),
            "lost" => transaction.lose_response(),
            _ => transaction.commit(),
        }
        let result = task.await.unwrap();
        if failure.ends_with("condition") {
            assert!(matches!(result, Err(StorageError::ConditionFailed(_))));
        } else {
            assert!(matches!(result, Err(StorageError::DynamoDB(_))));
        }
        if failure == "lost" {
            assert_eq!(
                f.snapshot(&keys).await,
                vec![
                    Some(json!({
                        "email_digest": f.email_digest, "subject": null,
                        "deleted_at": deleted_at, "reusable_after": deleted_at
                    })),
                    None
                ]
            );
        } else {
            assert_eq!(
                f.snapshot(&keys).await,
                before,
                "confirmed failure retains both rows"
            );
            if failure == "index-condition" {
                // Repair the intentionally corrupted fixture before exercising a normal retry.
                write(&f, &keys[1], original[1].clone().unwrap()).await;
            }
            // Earlier cleanup already permits a pending retry, but the active owner still blocks it.
            f.store
                .recreate_deleted_password_user(&f.email_digest, &f.email)
                .await
                .unwrap();
            let proof = f.proof().await;
            let pending_keys = f.keys(&proof, "user_unused");
            let pending = f.snapshot(&pending_keys).await;
            assert!(f.prepare(&f.store, &proof).await.is_none());
            assert_eq!(f.snapshot(&pending_keys).await, pending);
        }
        let retried = f
            .store
            .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
            .await
            .unwrap();
        assert_eq!(retried.account.deleted_at, Some(deleted_at));
        assert_eq!(
            f.snapshot(&keys).await,
            vec![
                Some(json!({
                    "email_digest": f.email_digest, "subject": null,
                    "deleted_at": deleted_at, "reusable_after": deleted_at
                })),
                None
            ]
        );
    }
}

pub async fn malformed_cleanup(storage: Arc<dyn StorageAdapter>) {
    for case in [
        "index-subject",
        "index-digest",
        "owner-binding",
        "owner-state",
        "owner-type",
    ] {
        let f = Fixture::new(storage.clone(), &format!("malformed-{case}@example.com")).await;
        let subject = complete(&f, &f.proof_digest).await;
        let keys = owner_keys(&f, &subject);
        let valid = f.snapshot(&keys).await;
        let index = usize::from(case.starts_with("index"));
        let mut invalid = valid[index].clone().unwrap();
        match case {
            "index-subject" => invalid["subject"] = json!("user_wrong-secret"),
            "index-digest" | "owner-binding" => invalid["email_digest"] = json!("wrong-secret"),
            "owner-state" => invalid["subject"] = Value::Null,
            _ => invalid["deleted_at"] = json!("invalid-secret"),
        }
        write(&f, &keys[index], invalid).await;
        let before = f.snapshot(&keys).await;
        let result = f
            .store
            .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
            .await;
        let Err(StorageError::DynamoDB(message)) = result else {
            panic!("malformed cleanup must fail operationally: {case}");
        };
        assert!(!message.contains("secret"));
        assert_eq!(f.snapshot(&keys).await, before, "{case}");
        assert_eq!(
            f.store.get_account(&subject).await.unwrap().unwrap().status,
            AccountStatus::Deleted
        );
        // Repair only the intentionally corrupt fixture, then retry the public deletion boundary.
        write(&f, &keys[index], valid[index].clone().unwrap()).await;
        f.store
            .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
            .await
            .unwrap();
        let rows = f.snapshot(&keys).await;
        assert!(rows[0].as_ref().unwrap()["subject"].is_null());
        assert!(rows[1].is_none());
    }
}
