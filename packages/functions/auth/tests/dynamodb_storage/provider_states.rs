use super::{dynamo, state_contract};
use irongate::{storage::StorageAdapter, store::provider_states::ProviderStateProof};
use std::sync::Arc;

async fn local<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    dynamo::run(|table| async move { test(Arc::new(table.storage)).await }).await;
}
state_contract::provider_state_cases!(
    local,
    ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"
);

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn provider_state_physical_ttl() {
    dynamo::run(|table| async move {
        let mut f = state_contract::Fixture::new(Arc::new(table.storage.clone()));
        f.create().await;
        let before = table.raw_item(&f.key).await.unwrap();
        assert_eq!(before.expiry, Some(f.record.expires_at.timestamp()));
        assert_eq!(before.value["browser_digest"], f.proof);
        assert!(f
            .store()
            .take_provider_state(f.key.sk(), ProviderStateProof::Apple)
            .await
            .unwrap()
            .is_none());
        assert!(table.raw_item(&f.key).await == Some(before));
        assert!(f
            .store()
            .take_provider_state(f.key.sk(), ProviderStateProof::GoogleBrowser(&f.proof))
            .await
            .unwrap()
            .is_some());
        assert!(table.raw_item(&f.key).await.is_none());
    })
    .await;
}
