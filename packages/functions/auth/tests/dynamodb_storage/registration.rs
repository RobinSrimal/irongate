use super::{dynamo, registration_contract, transaction_gate::PausedStorage};
use irongate::{
    core::passwords::hash_password_for_storage,
    storage::StorageAdapter,
    store::{keys::StoreKey, AuthStore},
};
use std::sync::Arc;

async fn local<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    dynamo::run(|table| async move { test(Arc::new(table.storage)).await }).await;
}

registration_contract::registration_cases!(
    local,
    ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"
);

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn registration_physical_ttl_and_persistent_activation() {
    dynamo::run(|table| async move {
        let f =
            registration_contract::Fixture::new(Arc::new(table.storage.clone()), "ttl@example.com")
                .await;
        let proof_key = irongate::store::keys::StoreKey::password_verification(&f.proof_digest);
        let proof = table.raw_item(&proof_key).await.unwrap();
        assert_eq!(proof.expiry, Some(f.expires_at.timestamp()));
        assert_eq!(proof.value["expires_at"], serde_json::json!(f.expires_at));
        let hash = hash_password_for_storage(registration_contract::FIRST).unwrap();
        let (paused, mut gate) =
            PausedStorage::when(table.storage.clone(), registration_contract::completion);
        let store = AuthStore::new(paused);
        let prepared = f.prepare(&store, &f.proof_digest).await.unwrap();
        let task =
            tokio::spawn(async move { store.finish_password_registration(prepared, &hash).await });
        let transaction = gate.wait().await;
        let rejected_subject = registration_contract::candidate(&transaction.operations);
        let keys = f.keys(&f.proof_digest, &rejected_subject);
        let user_key = StoreKey::password_user(&f.email_digest);
        let mut user = table.raw_item(&user_key).await.unwrap().value;
        user["updated_at"] = serde_json::json!(chrono::Utc::now() + chrono::Duration::seconds(1));
        table
            .storage
            .set(&[user_key.pk(), user_key.sk()], user, None)
            .await
            .unwrap();
        let mut before = Vec::new();
        for key in &keys {
            before.push(table.raw_item(key).await);
        }
        transaction.commit();
        assert!(task.await.unwrap().unwrap().is_none());
        for (key, expected) in keys.iter().zip(before) {
            assert!(
                table.raw_item(key).await == expected,
                "cancellation preserves physical rows and TTL"
            );
        }
        let prepared = f.prepare(&f.store, &f.proof_digest).await.unwrap();
        let subject = f
            .store
            .finish_password_registration(
                prepared,
                &hash_password_for_storage(registration_contract::FIRST).unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(table.raw_item(&proof_key).await.is_none());
        for key in &f.keys(&f.proof_digest, subject.as_str())[1..] {
            assert!(
                table.raw_item(key).await.unwrap().expiry.is_none(),
                "activated rows must not expire"
            );
        }
    })
    .await;
}
