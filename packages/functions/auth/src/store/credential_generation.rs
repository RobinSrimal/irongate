//! Active-account snapshots and guards for credential issuance and use.

use super::{
    keys::StoreKey,
    records::{AccountRecord, AccountStatus},
    to_value, AuthStore,
};
use crate::{
    core::subjects::Subject,
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};

impl AuthStore {
    pub async fn active_credential_account(
        &self,
        subject: &str,
    ) -> Result<Option<AccountRecord>, StorageError> {
        Ok(self
            .get_account(&Subject::from_persisted(subject.to_owned()))
            .await?
            .filter(|account| {
                account.subject == subject && account.status == AccountStatus::Active
            }))
    }

    pub(crate) async fn account_at_generation(
        &self,
        subject: &str,
        generation: u64,
    ) -> Result<Option<AccountRecord>, StorageError> {
        Ok(self
            .active_credential_account(subject)
            .await?
            .filter(|account| account.credential_generation == generation))
    }
}

pub(crate) fn equal(value: &impl serde::Serialize) -> Result<TransactCondition, StorageError> {
    Ok(TransactCondition::AttributeEquals {
        name: "value".into(),
        value: to_value(value)?,
    })
}

pub(crate) fn guard(account: &AccountRecord) -> Result<TransactOperation, StorageError> {
    Ok(TransactOperation::ConditionCheck {
        key: StoreKey::account(&account.subject).parts(),
        condition: equal(account)?,
    })
}
