//! OAuth authorization-code store operations.

use super::{to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::AuthorizationCodeRecord;
use chrono::Utc;

impl AuthStore {
    pub async fn create_authorization_code(
        &self,
        code_digest: &str,
        record: AuthorizationCodeRecord,
    ) -> Result<(), StorageError> {
        let key = StoreKey::authorization_code(code_digest);
        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: key.parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::Put {
                    key: key.parts(),
                    value: to_value(&record)?,
                    expiry: Some(record.expires_at),
                },
            ])
            .await
    }

    pub async fn take_authorization_code(
        &self,
        code_digest: &str,
    ) -> Result<Option<AuthorizationCodeRecord>, StorageError> {
        let key = StoreKey::authorization_code(code_digest);
        let record: AuthorizationCodeRecord = match self.get_record(&key).await? {
            Some(record) => record,
            None => return Ok(None),
        };

        if Utc::now() >= record.expires_at {
            self.remove_record(&key).await?;
            return Ok(None);
        }

        let result = self
            .storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: key.parts(),
                    condition: TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&record)?,
                    },
                },
                TransactOperation::Delete { key: key.parts() },
            ])
            .await;

        match result {
            Ok(()) => Ok(Some(record)),
            Err(StorageError::ConditionFailed(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
