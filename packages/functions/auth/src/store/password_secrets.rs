//! Password verification and reset secret store operations.

use super::{to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::EmailVerificationRecord;
use chrono::{DateTime, Utc};

impl<S> AuthStore<S>
where
    S: StorageAdapter,
{
    pub async fn create_email_verification(
        &self,
        verification_digest: &str,
        email_digest: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let record = EmailVerificationRecord {
            email_digest: email_digest.to_string(),
            purpose: "verify_email".to_string(),
            created_at: Utc::now(),
            expires_at,
        };
        let key = StoreKey::password_verification(verification_digest);

        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: key.parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::Put {
                    key: key.parts(),
                    value: to_value(&record)?,
                    expiry: Some(expires_at),
                },
            ])
            .await
    }

    pub async fn consume_email_verification(
        &self,
        verification_digest: &str,
    ) -> Result<Option<EmailVerificationRecord>, StorageError> {
        let key = StoreKey::password_verification(verification_digest);
        let record: EmailVerificationRecord = match self.get_record(&key).await? {
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
