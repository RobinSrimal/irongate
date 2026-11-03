#[path = "support/dynamo.rs"]
mod dynamo;

use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactCondition, TransactOperation},
    store::{records::AuthorizationCodeRecord, AuthStore},
};
use serde_json::{json, Value};

fn replace(key: &str, expected: Value, value: Value) -> TransactOperation {
    TransactOperation::Put {
        key: vec!["contract".into(), key.into()],
        value,
        expiry: None,
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
        let store = table.storage;
        store
            .set(&["contract", "a"], json!({"version":1}), None)
            .await
            .unwrap();
        store
            .set(&["contract", "b"], json!({"version":1}), None)
            .await
            .unwrap();
        store
            .transact(vec![replace(
                "a",
                json!({"version":1}),
                json!({"version":2}),
            )])
            .await
            .unwrap();
        assert_eq!(
            store.get(&["contract", "a"]).await.unwrap(),
            Some(json!({"version":2}))
        );

        let error = store
            .transact(vec![
                replace("a", json!({"version":1}), json!({"version":3})),
                replace("b", json!({"version":1}), json!({"version":2})),
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
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn stale_code_consumer_is_rejected_after_the_winner_commits() {
    dynamo::run(|table| async move {
        let store = AuthStore::new(table.storage);
        let record = AuthorizationCodeRecord {
            client_id: "web".into(),
            redirect_uri: "https://example.test/callback".into(),
            subject: "user_test".into(),
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
