use super::{dynamo, rate_contract};
use chrono::{Duration, Utc};
use irongate::{
    storage::StorageAdapter,
    store::{keys::StoreKey, rate_limits::email_identifier},
};
use std::sync::Arc;

async fn local<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    dynamo::run(|table| async move { test(Arc::new(table.storage)).await }).await;
}
rate_contract::rate_cases!(
    local,
    ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"
);

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn rate_counter_window_reset_replaces_physical_ttl() {
    dynamo::run(|table| async move {
        use rate_contract::*;
        let now = Utc::now();
        let key = StoreKey::new(
            format!("ratelimit\x1f{}", ENDPOINT.as_str()),
            email_identifier(SECRET, EMAIL).unwrap(),
        );
        check(&table.storage, &config(10, 1), now, IP, EMAIL)
            .await
            .unwrap();
        let first = table.raw_item(&key).await.unwrap();
        assert_eq!(
            first.expiry,
            Some((now + Duration::seconds(120)).timestamp())
        );
        check(
            &table.storage,
            &config(10, 1),
            now + Duration::seconds(60),
            IP,
            EMAIL,
        )
        .await
        .unwrap();
        let next = table.raw_item(&key).await.unwrap();
        assert_eq!(
            next.expiry,
            Some((now + Duration::seconds(180)).timestamp())
        );
        assert_eq!(next.value["count"], 1);
        assert_ne!(next.value["window_start"], first.value["window_start"]);
    })
    .await;
}
