#[path = "support/recovery.rs"]
#[allow(dead_code)]
mod recovery_fixture;
#[path = "support/registration_contract.rs"]
mod registration_contract;
mod support;
#[path = "support/transaction_gate.rs"]
#[allow(dead_code)]
mod transaction_gate;

use irongate::storage::StorageAdapter;
use std::sync::Arc;

async fn memory<F, Fut>(test: F)
where
    F: FnOnce(Arc<dyn StorageAdapter>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    test(Arc::new(support::TestStorage::new())).await;
}

registration_contract::registration_cases!(memory);
