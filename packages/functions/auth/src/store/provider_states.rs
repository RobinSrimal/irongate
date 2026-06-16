//! OIDC provider-state store operations.

use super::{to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::ProviderStateRecord;
use chrono::Utc;

impl AuthStore {
    pub async fn create_provider_state(
        &self,
        state_digest: &str,
        record: ProviderStateRecord,
    ) -> Result<(), StorageError> {
        let key = StoreKey::provider_state(state_digest);
        self.storage
            .transact(vec![TransactOperation::Put {
                key: key.parts(),
                value: to_value(&record)?,
                expiry: Some(record.expires_at),
                condition: Some(TransactCondition::NotExists),
            }])
            .await
    }

    pub async fn take_provider_state(
        &self,
        state_digest: &str,
    ) -> Result<Option<ProviderStateRecord>, StorageError> {
        let key = StoreKey::provider_state(state_digest);
        let record: ProviderStateRecord = match self.get_record(&key).await? {
            Some(record) => record,
            None => return Ok(None),
        };

        if Utc::now() >= record.expires_at {
            self.remove_record(&key).await?;
            return Ok(None);
        }

        let result = self
            .storage
            .transact(vec![TransactOperation::Delete {
                key: key.parts(),
                condition: Some(TransactCondition::AttributeEquals {
                    name: "value".to_string(),
                    value: to_value(&record)?,
                }),
            }])
            .await;

        match result {
            Ok(()) => Ok(Some(record)),
            Err(StorageError::ConditionFailed(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
