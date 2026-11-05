//! Email-verification proofs and indexed reset cleanup for account lifecycle operations.

use super::{credential_generation::equal, to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{EmailVerificationRecord, PasswordResetSubjectIndexRecord};
use chrono::{DateTime, Utc};

impl AuthStore {
    pub async fn create_email_verification(
        &self,
        verification_digest: &str,
        email_digest: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let user = self
            .get_password_user_by_email_digest(email_digest)
            .await?
            .filter(|user| user.is_pending_registration())
            .ok_or_else(|| StorageError::ConditionFailed("registration is not pending".into()))?;
        let record = EmailVerificationRecord {
            email_digest: email_digest.to_string(),
            registration_id: user
                .registration_id
                .clone()
                .expect("validated pending registration"),
            purpose: "verify_email".to_string(),
            created_at: (self.registration_now)(),
            expires_at,
        };
        let key = StoreKey::password_verification(verification_digest);

        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: StoreKey::password_user(email_digest).parts(),
                    condition: equal(&user)?,
                },
                TransactOperation::Put {
                    key: key.parts(),
                    value: to_value(&record)?,
                    expiry: Some(expires_at),
                    condition: Some(TransactCondition::NotExists),
                },
            ])
            .await
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
                        condition: None,
                    },
                    TransactOperation::Delete {
                        key: index_key.parts(),
                        condition: None,
                    },
                ])
                .await?;
            deleted += 1;
        }

        Ok(deleted)
    }
}
