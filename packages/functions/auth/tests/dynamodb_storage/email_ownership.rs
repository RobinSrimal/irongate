use super::{dynamo, email_ownership, registration_contract::Fixture};
use aws_sdk_dynamodb::types::AttributeValue;
use irongate::{
    error::StorageError,
    storage::StorageAdapter,
    store::{keys::StoreKey, records::AccountStatus, DeletedIdentityReusePolicy},
};
use std::{collections::HashMap, sync::Arc};

async fn local<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    dynamo::run(|table| async move { test(Arc::new(table.storage)).await }).await;
}

email_ownership::ownership_cases!(
    local,
    ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"
);

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn ownership_active_and_tombstone_rows_have_no_physical_ttl() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "ownership-ttl@example.com").await;
        let subject = email_ownership::complete(&f, &f.proof_digest).await;
        let keys = email_ownership::owner_keys(&f, &subject);
        for key in &keys {
            assert!(table.raw_item(key).await.unwrap().expiry.is_none());
        }
        let deleted = f
            .store
            .delete_account(&subject, DeletedIdentityReusePolicy::AfterRetention, 2)
            .await
            .unwrap();
        let owner = table.raw_item(&keys[0]).await.unwrap();
        assert!(owner.expiry.is_none(), "retention is not a TTL");
        assert_eq!(
            owner.value,
            serde_json::json!({
                "email_digest": f.email_digest, "subject": null,
                "deleted_at": deleted.account.deleted_at,
                "reusable_after": deleted.account.deleted_at.unwrap() + chrono::Duration::days(2),
            })
        );
        assert!(table.raw_item(&keys[1]).await.is_none());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn malformed_physical_rows_cannot_hide_unfinished_ownership_cleanup() {
    dynamo::run(|table| async move {
        for target in 0..2 {
            for case in ["missing", "non-string", "invalid-json"] {
                let f = Fixture::new(
                    Arc::new(table.storage.clone()),
                    &format!("raw-{target}-{case}@example.com"),
                )
                .await;
                let subject = email_ownership::complete(&f, &f.proof_digest).await;
                let keys = email_ownership::owner_keys(&f, &subject);
                let before = [
                    attributes(&table, &keys[0]).await.unwrap(),
                    attributes(&table, &keys[1]).await.unwrap(),
                ];
                let original = before[target].clone();
                let mut corrupt = original.clone();
                match case {
                    "missing" => {
                        corrupt.remove("value");
                    }
                    "non-string" => {
                        corrupt.insert("value".into(), AttributeValue::N("7".into()));
                    }
                    _ => {
                        corrupt.insert(
                            "value".into(),
                            AttributeValue::S("{ private-index-secret".into()),
                        );
                    }
                }
                table
                    .client
                    .put_item()
                    .table_name(&table.name)
                    .set_item(Some(corrupt.clone()))
                    .send()
                    .await
                    .unwrap();
                let key = &keys[target];
                assert_safe_decode_error(
                    table.storage.get(&[key.pk(), key.sk()]).await.unwrap_err(),
                );
                assert_safe_decode_error(
                    table
                        .storage
                        .query_prefix(&[key.pk(), key.sk()])
                        .await
                        .unwrap_err(),
                );
                let page_error = table
                    .storage
                    .query_prefix_page(&[key.pk(), key.sk()], 1, None)
                    .await
                    .unwrap_err();
                assert_safe_decode_error(page_error);
                let result = f
                    .store
                    .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
                    .await;
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("{case}: cleanup must not report success"),
                };
                assert_safe_decode_error(error);
                for (index, key) in keys.iter().enumerate() {
                    let expected = if index == target {
                        &corrupt
                    } else {
                        &before[index]
                    };
                    assert_eq!(attributes(&table, key).await.as_ref(), Some(expected));
                }
                let account = f.store.get_account(&subject).await.unwrap().unwrap();
                assert_eq!(account.status, AccountStatus::Deleted);

                // Repair the deliberately malformed physical item in this fixture-owned table only.
                table
                    .client
                    .put_item()
                    .table_name(&table.name)
                    .set_item(Some(original))
                    .send()
                    .await
                    .unwrap();
                let retry = f
                    .store
                    .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
                    .await
                    .unwrap();
                assert_eq!(retry.account.deleted_at, account.deleted_at);
                let owner = table.raw_item(&keys[0]).await.unwrap();
                assert_eq!(
                    owner.value,
                    serde_json::json!({
                        "email_digest": f.email_digest, "subject": null,
                        "deleted_at": account.deleted_at, "reusable_after": account.deleted_at,
                    })
                );
                assert!(owner.expiry.is_none());
                assert!(attributes(&table, &keys[1]).await.is_none());
            }
        }
    })
    .await;
}

fn assert_safe_decode_error(error: StorageError) {
    let StorageError::DynamoDB(message) = error else {
        panic!("malformed row is an operational storage error");
    };
    assert!(!message.contains("private-index-secret"));
    assert!(
        !message.contains("line 1"),
        "do not expose decoder diagnostics"
    );
}

async fn attributes(
    table: &dynamo::LocalTable,
    key: &StoreKey,
) -> Option<HashMap<String, AttributeValue>> {
    table
        .client
        .get_item()
        .table_name(&table.name)
        .consistent_read(true)
        .key("pk", AttributeValue::S(key.pk().into()))
        .key("sk", AttributeValue::S(key.sk().into()))
        .send()
        .await
        .unwrap()
        .item
}
