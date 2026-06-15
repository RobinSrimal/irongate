//! OAuth authorize-session store operations.

use super::{to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::AuthorizeSessionRecord;
use chrono::Utc;

impl<S> AuthStore<S>
where
    S: StorageAdapter,
{
    pub async fn create_authorize_session(
        &self,
        session_digest: &str,
        record: AuthorizeSessionRecord,
    ) -> Result<(), StorageError> {
        let key = StoreKey::authorize_session(session_digest);
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

    pub async fn take_authorize_session(
        &self,
        session_digest: &str,
    ) -> Result<Option<AuthorizeSessionRecord>, StorageError> {
        let key = StoreKey::authorize_session(session_digest);
        let record: AuthorizeSessionRecord = match self.get_record(&key).await? {
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
