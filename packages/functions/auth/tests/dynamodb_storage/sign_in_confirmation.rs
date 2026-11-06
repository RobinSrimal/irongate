use super::{confirmation_contract, dynamo};
use irongate::storage::StorageAdapter;
use std::sync::Arc;

async fn local<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    dynamo::run(|table| async move { test(Arc::new(table.storage)).await }).await;
}
confirmation_contract::confirmation_cases!(
    local,
    ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"
);

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn confirmation_physical_ttl_and_rollback() {
    dynamo::run(|table| async move {
        let f = confirmation_contract::Fixture::new(Arc::new(table.storage.clone()), "physical");
        f.create().await;
        let pending = table.raw_item(&f.key()).await.unwrap();
        let expiry: chrono::DateTime<chrono::Utc> =
            serde_json::from_value(pending.value["expires_at"].clone()).unwrap();
        assert_eq!(pending.expiry, Some(expiry.timestamp()));
        let code = super::provider_contract::code_digest();
        let subject = f.finish(&code).await.unwrap();
        let keys = f.provider.keys(subject.as_str(), &code);
        let issued = table.raw_item(&keys[3]).await.unwrap();
        let expiry: chrono::DateTime<chrono::Utc> =
            serde_json::from_value(issued.value["expires_at"].clone()).unwrap();
        assert_eq!(issued.expiry, Some(expiry.timestamp()));
        assert!(table.raw_item(&f.key()).await.is_none());
        for key in keys[..3].iter().chain(
            super::provider_contract::ownership::ownership_keys(subject.as_str(), &f.email).iter(),
        ) {
            assert!(table.raw_item(key).await.unwrap().expiry.is_none());
        }
        let mut loser =
            confirmation_contract::Fixture::new(Arc::new(table.storage.clone()), "physical-loser");
        loser.email = f.email.clone();
        loser.create().await;
        let keys = loser.keys(&code).await;
        let mut before = Vec::new();
        for key in &keys {
            before.push(table.raw_item(key).await);
        }
        super::provider_contract::rejected(loser.finish(&code).await);
        for (key, expected) in keys.iter().zip(before) {
            assert!(table.raw_item(key).await == expected);
        }
    })
    .await;
}
