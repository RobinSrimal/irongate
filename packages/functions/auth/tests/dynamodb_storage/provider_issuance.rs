use super::{dynamo, provider_contract};
use irongate::storage::StorageAdapter;
use std::sync::Arc;

async fn local<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    dynamo::run(|table| async move { test(Arc::new(table.storage)).await }).await;
}

provider_contract::provider_cases!(
    local,
    ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"
);

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn provider_physical_ttl_and_atomic_metadata() {
    dynamo::run(|table| async move {
        use irongate::store::{keys::StoreKey, IdentityProvider};
        use provider_contract::*;
        for provider in [IdentityProvider::Google, IdentityProvider::Apple] {
            let f = Fixture::new(Arc::new(table.storage.clone()), provider, "ttl-provider");
            let mut expected_subject = None;
            for kind in [Kind::New, Kind::Active] {
                *f.clock.lock().unwrap() += chrono::Duration::seconds(1);
                let code = code_digest();
                let props = properties("ttl@example.test", &format!("{kind:?}"));
                let subject = f.issue(props.clone(), &code).await;
                let record = f.assert_committed(&subject, &code, &props, 0).await;
                if let Some(expected) = &expected_subject {
                    assert_eq!(subject.as_str(), expected);
                }
                expected_subject = Some(subject.as_str().to_owned());
                let keys = f.keys(subject.as_str(), &code);
                let raw = table.raw_item(&keys[3]).await.unwrap();
                assert_eq!(raw.expiry, Some(record.expires_at.timestamp()));
                assert_eq!(
                    raw.value["expires_at"],
                    serde_json::json!(record.expires_at)
                );
                for key in &keys[..3] {
                    assert!(table.raw_item(key).await.unwrap().expiry.is_none());
                }
                if provider == IdentityProvider::Apple {
                    for key in apple::ownership_keys(subject.as_str(), "ttl@example.test") {
                        let raw = table.raw_item(&key).await.unwrap();
                        assert!(raw.expiry.is_none());
                        assert_eq!(raw.value["subject"], subject.as_str());
                    }
                }
                // A real code collision must preserve complete existing metadata and TTL.
                let mut before = Vec::new();
                for key in &keys {
                    before.push(table.raw_item(key).await);
                }
                let prepared = f
                    .prepare(
                        &f.store,
                        properties("rejected@example.test", "Rejected update"),
                        irongate::store::DeletedIdentityReusePolicy::Immediate,
                    )
                    .await
                    .unwrap();
                rejected(f.store.finish_provider_issuance(prepared, &code).await);
                for (key, expected) in keys.iter().zip(before) {
                    assert!(table.raw_item(key).await == expected);
                }
                assert!(f
                    .store
                    .take_authorization_code(&code)
                    .await
                    .unwrap()
                    .is_some());
                assert!(table
                    .raw_item(&StoreKey::authorization_code(&code))
                    .await
                    .is_none());
            }
        }
    })
    .await;
}
