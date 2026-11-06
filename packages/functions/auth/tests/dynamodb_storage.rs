#[path = "support/sign_in_confirmation.rs"]
mod confirmation_contract;
#[path = "support/dynamo.rs"]
mod dynamo;
#[path = "support/email_ownership.rs"]
mod email_ownership;
#[path = "dynamodb_storage/email_ownership.rs"]
mod ownership;
#[path = "support/provider_issuance.rs"]
mod provider_contract;
#[path = "dynamodb_storage/provider_issuance.rs"]
mod provider_issuance;
#[path = "dynamodb_storage/provider_states.rs"]
mod provider_states;
#[path = "support/rate_contract.rs"]
mod rate_contract;
#[path = "support/rate_control.rs"]
mod rate_control;
#[path = "dynamodb_storage/rate_limits.rs"]
mod rate_limits;
#[path = "dynamodb_storage/recovery.rs"]
mod recovery;
#[path = "support/recovery.rs"]
mod recovery_fixture;
#[path = "dynamodb_storage/recovery_races.rs"]
mod recovery_races;
#[path = "dynamodb_storage/refresh.rs"]
mod refresh;
#[path = "dynamodb_storage/registration.rs"]
mod registration;
#[path = "support/registration_contract.rs"]
mod registration_contract;
#[path = "dynamodb_storage/sign_in_confirmation.rs"]
mod sign_in_confirmation;
#[path = "support/provider_states.rs"]
mod state_contract;
mod support;
#[path = "support/transaction_gate.rs"]
mod transaction_gate;

use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactCondition, TransactOperation},
    store::{
        keys::StoreKey, records::AuthorizationCodeRecord, AuthStore, DeletedIdentityReusePolicy,
        IdentityProvider,
    },
};
use serde_json::{json, Value};

fn replace(
    key: &str,
    expected: Value,
    value: Value,
    expiry: Option<chrono::DateTime<chrono::Utc>>,
) -> TransactOperation {
    TransactOperation::Put {
        key: vec!["contract".into(), key.into()],
        value,
        expiry,
        condition: Some(TransactCondition::AttributeEquals {
            name: "value".into(),
            value: expected,
        }),
    }
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn conditional_replacement_and_failed_transaction_are_atomic() {
    dynamo::run(|table| async move {
        let store = &table.storage;
        let t0 = chrono::Utc::now() + chrono::Duration::hours(1);
        let t1 = t0 + chrono::Duration::hours(1);
        let old = json!({"version":1,"removed":true});
        store
            .set(&["contract", "a"], old.clone(), Some(t0))
            .await
            .unwrap();
        store
            .set(&["contract", "b"], json!({"version":1}), Some(t0))
            .await
            .unwrap();
        store
            .transact(vec![replace(
                "a",
                old.clone(),
                json!({"version":2}),
                Some(t1),
            )])
            .await
            .unwrap();
        assert_eq!(
            store.get(&["contract", "a"]).await.unwrap(),
            Some(json!({"version":2}))
        );
        let a = StoreKey::new("contract", "a");
        let b = StoreKey::new("contract", "b");
        let before_a = table.raw_item(&a).await.unwrap();
        let before_b = table.raw_item(&b).await.unwrap();
        assert_eq!(before_a.expiry, Some(t1.timestamp()));

        let error = store
            .transact(vec![
                replace("a", old, json!({"version":3}), None),
                replace("b", json!({"version":1}), json!({"version":2}), None),
                TransactOperation::Put {
                    key: vec!["contract".into(), "unrelated".into()],
                    value: json!(true),
                    expiry: None,
                    condition: None,
                },
            ])
            .await
            .unwrap_err();
        assert!(matches!(error, StorageError::ConditionFailed(_)));
        assert_eq!(
            store.get(&["contract", "a"]).await.unwrap(),
            Some(json!({"version":2}))
        );
        assert_eq!(
            store.get(&["contract", "b"]).await.unwrap(),
            Some(json!({"version":1}))
        );
        assert_eq!(store.get(&["contract", "unrelated"]).await.unwrap(), None);
        assert!(table.raw_item(&a).await.unwrap() == before_a);
        assert!(table.raw_item(&b).await.unwrap() == before_b);
        store
            .transact(vec![replace(
                "a",
                json!({"version":2}),
                json!({"version":4}),
                None,
            )])
            .await
            .unwrap();
        assert!(table.raw_item(&a).await.unwrap().expiry.is_none());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn account_lifecycle_replacements_keep_accounts_persistent() {
    dynamo::run(|table| async move {
        let store = AuthStore::new(table.storage.clone());
        let subject = store
            .create_account_with_identity(IdentityProvider::Password, "identity", json!({}))
            .await
            .unwrap();
        let key = StoreKey::account(subject.as_str());
        let disabled = store.disable_account(&subject).await.unwrap();
        assert!(disabled.disabled_at.is_some());
        assert!(table.raw_item(&key).await.unwrap().expiry.is_none());
        let enabled = store.enable_account(&subject).await.unwrap();
        assert!(enabled.disabled_at.is_none());
        assert!(table.raw_item(&key).await.unwrap().expiry.is_none());
        store
            .delete_account(&subject, DeletedIdentityReusePolicy::Never, 0)
            .await
            .unwrap();
        let deleted = table.raw_item(&key).await.unwrap();
        assert_eq!(deleted.value["status"], "deleted");
        assert!(deleted.expiry.is_none());
        assert!(matches!(
            store.enable_account(&subject).await,
            Err(StorageError::ConditionFailed(_))
        ));
        assert!(table.raw_item(&key).await.unwrap() == deleted);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn stale_code_consumer_is_rejected_after_the_winner_commits() {
    dynamo::run(|table| async move {
        let store = AuthStore::new(table.storage);
        let subject = store
            .create_account_with_identity(IdentityProvider::Password, "identity", json!({}))
            .await
            .unwrap();
        let record = AuthorizationCodeRecord {
            credential_generation: 0,
            client_id: "web".into(),
            redirect_uri: "https://example.test/callback".into(),
            subject: subject.as_str().into(),
            subject_type: "user".into(),
            properties: json!({}),
            code_challenge: None,
            code_challenge_method: None,
            scope: "openid".into(),
            oidc_nonce: None,
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        };
        store
            .create_authorization_code("digest", record)
            .await
            .unwrap();
        // Both consumers read before either writes. Order their commits to exercise a
        // stale conditional delete deterministically, not an SDK retry of a live conflict.
        let first = store
            .get_authorization_code("digest")
            .await
            .unwrap()
            .unwrap();
        let second = store
            .get_authorization_code("digest")
            .await
            .unwrap()
            .unwrap();
        assert!(store
            .delete_authorization_code_if_current("digest", first)
            .await
            .unwrap()
            .is_some());
        assert!(store
            .delete_authorization_code_if_current("digest", second)
            .await
            .unwrap()
            .is_none());
        assert!(store
            .take_authorization_code("digest")
            .await
            .unwrap()
            .is_none());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn duplicate_item_transaction_is_an_operational_error() {
    dynamo::run(|table| async move {
        table
            .storage
            .set(&["contract", "a"], json!(1), None)
            .await
            .unwrap();
        let error = table
            .storage
            .transact(vec![
                TransactOperation::Put {
                    key: vec!["contract".into(), "a".into()],
                    value: json!(2),
                    expiry: None,
                    condition: None,
                },
                TransactOperation::Delete {
                    key: vec!["contract".into(), "a".into()],
                    condition: None,
                },
            ])
            .await
            .unwrap_err();
        assert!(matches!(error, StorageError::DynamoDB(_)), "{error:?}");
        assert_eq!(
            table.storage.get(&["contract", "a"]).await.unwrap(),
            Some(json!(1))
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn cleanup_after_success_and_panic_deletes_only_the_owned_table() {
    dynamo::run(|outer| async move {
        for panic in [false, true] {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(dynamo::run(move |inner| async move {
                sender.send(inner.name).unwrap();
                assert!(!panic, "intentional fixture cleanup probe");
            }));
            let deleted_name = receiver.await.unwrap();
            let result = task.await;
            assert_eq!(result.is_err(), panic);
            let error = outer
                .client
                .describe_table()
                .table_name(deleted_name)
                .send()
                .await
                .unwrap_err();
            assert!(error
                .as_service_error()
                .is_some_and(|error| error.is_resource_not_found_exception()));
            outer
                .client
                .describe_table()
                .table_name(&outer.name)
                .send()
                .await
                .expect("other fixture table survives cleanup");
        }
    })
    .await;
}
