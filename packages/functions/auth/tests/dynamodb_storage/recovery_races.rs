use super::{
    dynamo,
    recovery::{puts, snapshot},
    recovery_fixture::{Fixture, NEW, OLD},
    transaction_gate::PausedStorage,
};
use irongate::{
    core::subjects::Subject,
    error::StorageError,
    providers::password::{
        PasswordLoginError, PasswordResetCompleteError, PasswordResetRequestStatus,
    },
    storage::TransactOperation,
    store::{
        keys::StoreKey, records::AccountStatus, refresh::RefreshTokenStoreError, AuthStore,
        DeletedIdentityReusePolicy,
    },
};
use std::sync::Arc;

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn concurrent_resets_have_exactly_one_commit() {
    dynamo::run(|table| async move {
        for same_link in [true, false] {
            let f = Fixture::new(
                Arc::new(table.storage.clone()),
                &format!("race-{same_link}@example.com"),
            )
            .await;
            let first = f.request().await;
            let second = if same_link {
                first.clone()
            } else {
                f.request().await
            };
            let (paused, mut gate) =
                PausedStorage::when(table.storage.clone(), |ops| puts(ops, "password:user"));
            let store = AuthStore::new(paused);
            let (result, committed) = tokio::join!(f.complete(&store, &first), async {
                let pending = gate.wait().await;
                f.complete(&f.store, &second).await.unwrap();
                let committed = snapshot(&table, &f).await;
                pending.commit();
                committed
            });
            assert!(matches!(
                result,
                Err(PasswordResetCompleteError::InvalidResetToken)
            ));
            assert!(
                snapshot(&table, &f).await == committed,
                "losing completion made no writes"
            );
            assert_eq!(f.generation().await, 1);
            assert!(f.password_is(NEW).await);
            assert!(f.complete(&f.store, &first).await.is_err());
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_and_rotation_cannot_leave_usable_old_sessions() {
    dynamo::run(|table| async move {
        for reset_first in [true, false] {
            let f = Fixture::new(
                Arc::new(table.storage.clone()),
                &format!("rotation-{reset_first}@example.com"),
            )
            .await;
            let link = f.request().await;
            let first = f.refresh("web").await;
            let (paused, mut gate) = PausedStorage::when(table.storage.clone(), move |ops| {
                puts(
                    ops,
                    if reset_first {
                        "oauth:refresh"
                    } else {
                        "password:user"
                    },
                )
            });
            let store = AuthStore::new(paused);
            if reset_first {
                let (result, committed) =
                    tokio::join!(f.rotate(&store, &first.raw_token, "web"), async {
                        let pending = gate.wait().await;
                        f.complete(&f.store, &link).await.unwrap();
                        let committed = snapshot(&table, &f).await;
                        pending.commit();
                        committed
                    });
                assert!(matches!(
                    result,
                    Err(RefreshTokenStoreError::SubjectInactive)
                ));
                assert!(
                    snapshot(&table, &f).await == committed,
                    "no losing successor or TTL writes"
                );
            } else {
                let (result, rotated) = tokio::join!(f.complete(&store, &link), async {
                    let pending = gate.wait().await;
                    let rotated = f.rotate(&f.store, &first.raw_token, "web").await.unwrap();
                    pending.commit();
                    rotated
                });
                result.unwrap();
                assert!(matches!(
                    f.rotate(&f.store, &rotated.raw_token, "web").await,
                    Err(RefreshTokenStoreError::SubjectInactive)
                ));
            }
            assert!(f.rotate(&f.store, &first.raw_token, "web").await.is_err());
            let fresh = f.refresh("web").await;
            f.rotate(&f.store, &fresh.raw_token, "web").await.unwrap();
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_rejects_stale_password_checks_even_before_account_read() {
    dynamo::run(|table| async move {
        for before_account in [false, true] {
            let f = Fixture::new(
                Arc::new(table.storage.clone()),
                &format!("login-{before_account}@example.com"),
            )
            .await;
            let link = f.request().await;
            let (paused, mut gate) = if before_account {
                PausedStorage::before_read(
                    table.storage.clone(),
                    StoreKey::account(&f.subject).parts(),
                )
            } else {
                PausedStorage::when(table.storage.clone(), |ops| puts(ops, "oauth:code"))
            };
            let store = AuthStore::new(paused);
            let (result, committed) = tokio::join!(f.login(&store, OLD), async {
                let pending = gate.wait().await;
                f.complete(&f.store, &link).await.unwrap();
                let committed = snapshot(&table, &f).await;
                pending.commit();
                committed
            });
            assert!(matches!(
                result,
                Err(PasswordLoginError::InvalidCredentials)
            ));
            assert!(
                snapshot(&table, &f).await == committed,
                "no code from a stale password"
            );
            let code = f.login(&f.store, NEW).await.unwrap();
            assert!(f
                .store
                .take_authorization_code(&f.code_digest(&code))
                .await
                .unwrap()
                .is_some());
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_guards_code_consumption_and_followup_refresh_creation() {
    dynamo::run(|table| async move {
        for consumed in [false, true] {
            let f = Fixture::new(
                Arc::new(table.storage.clone()),
                &format!("exchange-{consumed}@example.com"),
            )
            .await;
            let link = f.request().await;
            let code = f.login(&f.store, OLD).await.unwrap();
            let digest = f.code_digest(&code);
            if consumed {
                let code = f
                    .store
                    .take_authorization_code(&digest)
                    .await
                    .unwrap()
                    .unwrap();
                let (paused, mut gate) = PausedStorage::when(table.storage.clone(), |ops| {
                    puts(ops, "oauth:refresh_family")
                });
                let store = AuthStore::new(paused);
                let create = f.create_refresh(&store, "web", code.credential_generation);
                let (result, committed) = tokio::join!(create, async {
                    let pending = gate.wait().await;
                    f.complete(&f.store, &link).await.unwrap();
                    let committed = snapshot(&table, &f).await;
                    pending.commit();
                    committed
                });
                assert!(matches!(
                    result,
                    Err(RefreshTokenStoreError::SubjectInactive)
                ));
                assert!(
                    snapshot(&table, &f).await == committed,
                    "no stale refresh family or indexes"
                );
            } else {
                let (paused, mut gate) = PausedStorage::when(table.storage.clone(), |ops| {
                    ops.iter().any(|op| match op {
                        TransactOperation::Delete { key, .. } => {
                            key.first().is_some_and(|pk| pk == "oauth:code")
                        }
                        _ => false,
                    })
                });
                let store = AuthStore::new(paused);
                let (result, committed) =
                    tokio::join!(store.take_authorization_code(&digest), async {
                        let pending = gate.wait().await;
                        f.complete(&f.store, &link).await.unwrap();
                        let committed = snapshot(&table, &f).await;
                        pending.commit();
                        committed
                    });
                assert!(result.unwrap().is_none());
                assert!(snapshot(&table, &f).await == committed);
            }
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_prevents_stale_reset_link_creation_and_delivery() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "issue@example.com").await;
        let link = f.request().await;
        let mails = f.mailbox.len();
        let (paused, mut gate) =
            PausedStorage::when(table.storage.clone(), |ops| puts(ops, "password:reset"));
        let store = AuthStore::new(paused);
        let (result, committed) = tokio::join!(f.request_with(&store), async {
            let pending = gate.wait().await;
            f.complete(&f.store, &link).await.unwrap();
            let committed = snapshot(&table, &f).await;
            pending.commit();
            committed
        });
        assert_eq!(
            result.unwrap().status,
            PasswordResetRequestStatus::ResetEmailSent
        );
        assert_eq!(f.mailbox.len(), mails, "no email for rejected link write");
        assert!(snapshot(&table, &f).await == committed);
        let fresh = f.request().await;
        f.complete(&f.store, &fresh).await.unwrap();
    })
    .await;
}

async fn change_status(
    store: &AuthStore,
    subject: &Subject,
    delete: bool,
) -> Result<(), StorageError> {
    if delete {
        store
            .delete_account(subject, DeletedIdentityReusePolicy::Never, 0)
            .await
            .map(|_| ())
    } else {
        store.disable_account(subject).await.map(|_| ())
    }
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_and_lifecycle_writes_cannot_restore_stale_account_state() {
    dynamo::run(|table| async move {
        for delete in [false, true] {
            for reset_first in [false, true] {
                let f = Fixture::new(
                    Arc::new(table.storage.clone()),
                    &format!("lifecycle-{delete}-{reset_first}@example.com"),
                )
                .await;
                let subject = Subject::from_persisted(f.subject.clone());
                let link = f.request().await;
                if reset_first {
                    let pk = StoreKey::account(&f.subject).pk().to_owned();
                    let (paused, mut gate) =
                        PausedStorage::when(table.storage.clone(), move |ops| puts(ops, &pk));
                    let store = AuthStore::new(paused);
                    let (result, committed) =
                        tokio::join!(change_status(&store, &subject, delete), async {
                            let pending = gate.wait().await;
                            f.complete(&f.store, &link).await.unwrap();
                            let committed = snapshot(&table, &f).await;
                            pending.commit();
                            committed
                        });
                    assert!(matches!(result, Err(StorageError::ConditionFailed(_))));
                    assert!(snapshot(&table, &f).await == committed);
                    change_status(&f.store, &subject, delete).await.unwrap();
                } else {
                    let (paused, mut gate) = PausedStorage::when(table.storage.clone(), |ops| {
                        puts(ops, "password:user")
                    });
                    let store = AuthStore::new(paused);
                    let (result, committed) = tokio::join!(f.complete(&store, &link), async {
                        let pending = gate.wait().await;
                        change_status(&f.store, &subject, delete).await.unwrap();
                        let committed = snapshot(&table, &f).await;
                        pending.commit();
                        committed
                    });
                    assert!(matches!(
                        result,
                        Err(PasswordResetCompleteError::InvalidResetToken)
                    ));
                    assert!(
                        snapshot(&table, &f).await == committed,
                        "no reactivation or password resurrection"
                    );
                }
                let account = f.store.get_account(&subject).await.unwrap().unwrap();
                assert_eq!(
                    account.status,
                    if delete {
                        AccountStatus::Deleted
                    } else {
                        AccountStatus::Disabled
                    }
                );
                assert_eq!(account.credential_generation, u64::from(reset_first));
                assert!(f.login(&f.store, NEW).await.is_err());
                let mails = f.mailbox.len();
                f.request_with(&f.store).await.unwrap();
                assert_eq!(f.mailbox.len(), mails);
            }
        }
    })
    .await;
}
