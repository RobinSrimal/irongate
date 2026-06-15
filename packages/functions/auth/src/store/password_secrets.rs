//! Password verification and reset secret store operations.

use super::{to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{
    EmailVerificationRecord, PasswordResetRecord, PasswordResetSubjectIndexRecord,
};
use chrono::{DateTime, Utc};

impl AuthStore {
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

    pub async fn create_password_reset(
        &self,
        reset_digest: &str,
        email_digest: &str,
        subject: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let record = PasswordResetRecord {
            email_digest: email_digest.to_string(),
            subject: subject.to_string(),
            purpose: "reset_password".to_string(),
            created_at: Utc::now(),
            expires_at,
        };
        let key = StoreKey::password_reset(reset_digest);
        let index = PasswordResetSubjectIndexRecord {
            reset_digest: reset_digest.to_string(),
            subject: subject.to_string(),
            expires_at,
        };

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
                TransactOperation::Put {
                    key: StoreKey::password_reset_by_subject(subject, reset_digest).parts(),
                    value: to_value(&index)?,
                    expiry: Some(expires_at),
                },
            ])
            .await
    }

    pub async fn consume_password_reset(
        &self,
        reset_digest: &str,
    ) -> Result<Option<PasswordResetRecord>, StorageError> {
        let key = StoreKey::password_reset(reset_digest);
        let record: PasswordResetRecord = match self.get_record(&key).await? {
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
                TransactOperation::Delete {
                    key: StoreKey::password_reset_by_subject(&record.subject, reset_digest).parts(),
                },
            ])
            .await;

        match result {
            Ok(()) => Ok(Some(record)),
            Err(StorageError::ConditionFailed(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn delete_password_secrets_for_subject(
        &self,
        subject: &str,
    ) -> Result<usize, StorageError> {
        let index_pk = StoreKey::password_reset_by_subject_pk(subject);
        let rows = self.storage.query_prefix(&[index_pk.as_str()]).await?;
        let mut deleted = 0;

        for (_, value) in rows {
            let index: PasswordResetSubjectIndexRecord = serde_json::from_value(value)
                .map_err(|err| StorageError::DynamoDB(err.to_string()))?;
            let reset_key = StoreKey::password_reset(&index.reset_digest);
            let index_key = StoreKey::password_reset_by_subject(subject, &index.reset_digest);

            self.storage
                .transact(vec![
                    TransactOperation::Delete {
                        key: reset_key.parts(),
                    },
                    TransactOperation::Delete {
                        key: index_key.parts(),
                    },
                ])
                .await?;
            deleted += 1;
        }

        Ok(deleted)
    }
}
