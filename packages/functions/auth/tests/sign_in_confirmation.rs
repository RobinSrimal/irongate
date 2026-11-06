#[path = "support/sign_in_confirmation.rs"]
mod confirmation_contract;
#[path = "support/provider_issuance.rs"]
#[allow(dead_code, unused_imports, unused_macros)]
mod provider_contract;
mod support;
#[path = "support/transaction_gate.rs"]
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
confirmation_contract::confirmation_cases!(memory);
