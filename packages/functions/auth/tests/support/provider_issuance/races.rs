use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub async fn competing_callbacks(storage: Arc<dyn StorageAdapter>) {
    for kind in [Kind::New, Kind::Active, Kind::Deleted] {
        for winner in [0, 1] {
            let f = Fixture::new(
                storage.clone(),
                IdentityProvider::Google,
                &format!("race-{kind:?}-{winner}"),
            );
            let old = f.seed(kind).await;
            let codes = [code_digest(), code_digest()];
            let props = [
                properties("first@example.test", "First"),
                properties("second@example.test", "Second"),
            ];
            let (first_tx, first_task) = f.pause(props[0].clone(), &codes[0]).await;
            let (second_tx, second_task) = f.pause(props[1].clone(), &codes[1]).await;
            let first_keys = plan_keys(&f, &codes[0], &first_tx.operations, kind);
            let second_keys = plan_keys(&f, &codes[1], &second_tx.operations, kind);
            let (win_tx, win_task, lose_tx, lose_task, loser_keys) = if winner == 0 {
                (
                    first_tx,
                    first_task,
                    second_tx,
                    second_task,
                    second_keys.clone(),
                )
            } else {
                (
                    second_tx,
                    second_task,
                    first_tx,
                    first_task,
                    first_keys.clone(),
                )
            };
            win_tx.commit();
            let subject = win_task.await.unwrap().unwrap();
            let keys = [first_keys, second_keys].concat();
            let before_loser = f.rows(&keys).await;
            lose_tx.commit();
            rejected(lose_task.await.unwrap());
            assert_eq!(
                f.rows(&keys).await,
                before_loser,
                "stale callback cannot overwrite winner or leave orphans"
            );
            let rows = f.rows(&loser_keys).await;
            assert!(rows[3].is_none());
            if kind != Kind::Active {
                assert!(rows[0].is_none() && rows[2].is_none());
            }
            if let Some(old) = old {
                if kind == Kind::Active {
                    assert_eq!(subject.as_str(), old.as_str());
                } else {
                    assert_ne!(subject.as_str(), old.as_str());
                    assert_eq!(
                        f.store.get_account(&old).await.unwrap().unwrap().status,
                        AccountStatus::Deleted
                    );
                    assert!(f
                        .rows(&f.keys(old.as_str(), &codes[1 - winner])[2..3])
                        .await[0]
                        .is_none());
                }
            }
            f.assert_committed(&subject, &codes[winner], &props[winner], 0)
                .await;
            assert!(f
                .store
                .take_authorization_code(&codes[1 - winner])
                .await
                .unwrap()
                .is_none());
            assert!(f
                .store
                .take_authorization_code(&codes[winner])
                .await
                .unwrap()
                .is_some());
        }
    }
}

async fn competing_change(f: &Fixture, subject: &Subject, change: &str) {
    let account = StoreKey::account(subject.as_str());
    match change {
        "disable" => {
            f.store.disable_account(subject).await.unwrap();
        }
        "delete" => {
            f.store
                .delete_account(subject, DeletedIdentityReusePolicy::Immediate, 0)
                .await
                .unwrap();
        }
        "missing_account" => {
            f.storage
                .remove(&[account.pk(), account.sk()])
                .await
                .unwrap();
        }
        "generation" => {
            let mut value = f
                .rows(std::slice::from_ref(&account))
                .await
                .pop()
                .unwrap()
                .unwrap();
            value["credential_generation"] = json!(1);
            f.replace(&account, value).await;
        }
        "identity_update" => {
            let key = f.identity_key();
            let mut value = f
                .rows(std::slice::from_ref(&key))
                .await
                .pop()
                .unwrap()
                .unwrap();
            value["properties"] = properties("competing@example.test", "Competing update");
            f.replace(&key, value).await;
        }
        "identity_delete" => {
            f.store
                .delete_identity(f.provider, &f.digest, Utc::now())
                .await
                .unwrap();
        }
        _ => unreachable!(),
    }
}

pub async fn lifecycle_and_identity_races(storage: Arc<dyn StorageAdapter>) {
    for change in [
        "disable",
        "delete",
        "missing_account",
        "generation",
        "identity_update",
        "identity_delete",
    ] {
        for issuance_first in [false, true] {
            let f = Fixture::new(
                storage.clone(),
                IdentityProvider::Apple,
                &format!("lifecycle-{change}-{issuance_first}"),
            );
            let subject = f.seed(Kind::Active).await.unwrap();
            let code = code_digest();
            let props = properties("incoming@example.test", "Incoming update");
            let (tx, task) = f.pause(props.clone(), &code).await;
            let keys = plan_keys(&f, &code, &tx.operations, Kind::Active);
            if issuance_first {
                tx.commit();
                assert_eq!(task.await.unwrap().unwrap().as_str(), subject.as_str());
                f.assert_committed(&subject, &code, &props, 0).await;
                competing_change(&f, &subject, change).await;
                // Existing code consumption guards the account, not an identity-history epoch.
                let code_usable = change.starts_with("identity_");
                assert_eq!(
                    f.store
                        .take_authorization_code(&code)
                        .await
                        .unwrap()
                        .is_some(),
                    code_usable
                );
            } else {
                competing_change(&f, &subject, change).await;
                let before = f.rows(&keys).await;
                tx.commit();
                rejected(task.await.unwrap());
                assert_eq!(
                    f.rows(&keys).await,
                    before,
                    "stale issuance cannot undo {change}"
                );
                assert!(f
                    .store
                    .take_authorization_code(&code)
                    .await
                    .unwrap()
                    .is_none());
            }
        }
    }
}

pub async fn expiry_boundaries(storage: Arc<dyn StorageAdapter>) {
    for kind in [Kind::New, Kind::Active] {
        let f = Fixture::new(
            storage.clone(),
            IdentityProvider::Google,
            &format!("expiry-{kind:?}"),
        );
        let old = f.seed(kind).await;
        let code = code_digest();
        let mut keys = vec![f.identity_key(), StoreKey::authorization_code(&code)];
        if let Some(old) = old {
            keys.extend(f.keys(old.as_str(), &code));
        }
        let before = f.rows(&keys).await;
        *f.clock.lock().unwrap() = f.session.expires_at;
        rejected(
            f.prepare(&f.store, json!({}), DeletedIdentityReusePolicy::Immediate)
                .await,
        );
        assert_eq!(f.rows(&keys).await, before);
        for final_check in [false, true] {
            *f.clock.lock().unwrap() = f.session.expires_at - Duration::nanoseconds(1);
            let prepared = f
                .prepare(
                    &f.store,
                    properties("expired@example.test", "Expired"),
                    DeletedIdentityReusePolicy::Immediate,
                )
                .await
                .unwrap();
            if final_check {
                let calls = AtomicUsize::new(0);
                let expiry = f.session.expires_at;
                let store = f.store.clone().with_provider_issuance_clock(move || {
                    if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        expiry - Duration::nanoseconds(1)
                    } else {
                        expiry
                    }
                });
                rejected(store.finish_provider_issuance(prepared, &code).await);
            } else {
                *f.clock.lock().unwrap() = f.session.expires_at;
                rejected(f.store.finish_provider_issuance(prepared, &code).await);
            }
            assert_eq!(
                f.rows(&keys).await,
                before,
                "logical expiry, without TTL cleanup or sleeps"
            );
        }
        *f.clock.lock().unwrap() = f.session.created_at;
        let prepared = f
            .prepare(&f.store, json!({}), DeletedIdentityReusePolicy::Immediate)
            .await
            .unwrap();
        rejected(f.store.finish_provider_issuance(prepared, "").await);
        assert_eq!(f.rows(&keys).await, before);
    }
    let mut f = Fixture::new(storage, IdentityProvider::Apple, "timestamp-overflow");
    f.session.expires_at = DateTime::<Utc>::MAX_UTC;
    *f.clock.lock().unwrap() = f.session.expires_at - Duration::seconds(1);
    let prepared = f
        .prepare(
            &f.store,
            properties("overflow@example.test", "Overflow"),
            DeletedIdentityReusePolicy::Immediate,
        )
        .await
        .unwrap();
    let code = code_digest();
    assert!(matches!(
        f.store.finish_provider_issuance(prepared, &code).await,
        Err(StorageError::DynamoDB(_))
    ));
    assert!(f
        .rows(&[f.identity_key(), StoreKey::authorization_code(&code)])
        .await
        .iter()
        .all(Option::is_none));
}

pub async fn uncertain_outcomes(storage: Arc<dyn StorageAdapter>) {
    for kind in [Kind::New, Kind::Active, Kind::Deleted] {
        for lost_response in [false, true] {
            let f = Fixture::new(
                storage.clone(),
                IdentityProvider::Apple,
                &format!("fault-{kind:?}-{lost_response}"),
            );
            f.seed(kind).await;
            let code = code_digest();
            let props = properties("fault@example.test", "Attempted update");
            let (tx, task) = f.pause(props.clone(), &code).await;
            let keys = plan_keys(&f, &code, &tx.operations, kind);
            let subject = Subject::from_persisted(candidate(&tx.operations));
            let before = f.rows(&keys).await;
            if lost_response {
                tx.lose_response();
            } else {
                tx.fail_before_dispatch();
            }
            assert!(
                matches!(task.await.unwrap(), Err(StorageError::DynamoDB(_))),
                "operational errors must not become stale credentials"
            );
            if lost_response {
                f.assert_committed(&subject, &code, &props, 0).await;
                assert!(
                    f.store
                        .take_authorization_code(&code)
                        .await
                        .unwrap()
                        .is_some(),
                    "lost response is not rollback"
                );
            } else {
                assert_eq!(f.rows(&keys).await, before);
                assert!(f
                    .store
                    .get_authorization_code(&code)
                    .await
                    .unwrap()
                    .is_none());
            }
        }
    }
    let f = Fixture::new(storage.clone(), IdentityProvider::Google, "read-failure");
    let (paused, mut gate) = PausedStorage::before_read(storage, f.identity_key().parts());
    let store = AuthStore::new(paused);
    let session = f.session.clone();
    let digest = f.digest.clone();
    let task = tokio::spawn(async move {
        store
            .prepare_provider_issuance(
                ProviderIdentityInput {
                    provider: IdentityProvider::Google,
                    apple_email: None,
                    identity_digest: &digest,
                    properties: json!({}),
                },
                &session,
                TTL,
                DeletedIdentityReusePolicy::Immediate,
            )
            .await
    });
    gate.wait().await.fail_before_dispatch();
    assert!(matches!(
        task.await.unwrap(),
        Err(StorageError::DynamoDB(_))
    ));
    assert!(f.rows(&[f.identity_key()]).await[0].is_none());
}
