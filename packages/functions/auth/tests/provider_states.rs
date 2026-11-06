#[path = "support/provider_states.rs"]
mod state_contract;
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
state_contract::provider_state_cases!(memory);
