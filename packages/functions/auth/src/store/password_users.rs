//! Pending password registrations and credential tombstones.

use super::{credential_generation::equal, to_value, AuthStore};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{PasswordUserRecord, PasswordUserSubjectIndexRecord};
use chrono::Utc;

impl PasswordUserRecord {
    pub(crate) fn is_pending_registration(&self) -> bool {
        self.registration_id
            .as_deref()
            .is_some_and(|id| !id.is_empty())
            && self.email.as_deref().is_some_and(|email| !email.is_empty())
            && self.subject.is_none()
            && self.password_hash.is_none()
            && self.password_hash_updated_at.is_none()
            && !self.verified
            && self.deleted_at.is_none()
    }
}

impl AuthStore {
    fn pending_password_user(&self, email: &str) -> PasswordUserRecord {
        let now = (self.registration_now)();
        PasswordUserRecord {
            registration_id: Some(uuid::Uuid::new_v4().to_string()),
            email: Some(email.to_string()),
            subject: None,
            password_hash: None,
            password_hash_updated_at: None,
            verified: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    pub async fn create_pending_password_user(
        &self,
        email_digest: &str,
        email: &str,
    ) -> Result<(), StorageError> {
        self.storage
            .transact(vec![TransactOperation::Put {
                key: StoreKey::password_user(email_digest).parts(),
                value: to_value(&self.pending_password_user(email))?,
                expiry: None,
                condition: Some(TransactCondition::NotExists),
            }])
            .await
    }

    pub async fn get_password_user_by_email_digest(
        &self,
        email_digest: &str,
    ) -> Result<Option<PasswordUserRecord>, StorageError> {
        self.get_record(&StoreKey::password_user(email_digest))
            .await
    }

    pub async fn recreate_deleted_password_user(
        &self,
        email_digest: &str,
        email: &str,
    ) -> Result<(), StorageError> {
        let key = StoreKey::password_user(email_digest);
        let existing: PasswordUserRecord = self
            .get_record(&key)
            .await?
            .ok_or_else(|| StorageError::NotFound("password user not found".into()))?;
        if existing.deleted_at.is_none() {
            return Err(StorageError::ConditionFailed(
                "password user is not deleted".into(),
            ));
        }
        self.storage
            .transact(vec![TransactOperation::Put {
                key: key.parts(),
                value: to_value(&self.pending_password_user(email))?,
                expiry: None,
                condition: Some(equal(&existing)?),
            }])
            .await
    }

    pub async fn tombstone_password_users_for_subject(
        &self,
        subject: &str,
        deleted_at: chrono::DateTime<Utc>,
    ) -> Result<usize, StorageError> {
        let index_pk = StoreKey::password_user_by_subject_pk(subject);
        let rows = self.storage.query_prefix(&[index_pk.as_str()]).await?;
        let mut deleted = 0;

        for (_, value) in rows {
            let index: PasswordUserSubjectIndexRecord = serde_json::from_value(value)
                .map_err(|err| StorageError::DynamoDB(err.to_string()))?;
            let user_key = StoreKey::password_user(&index.email_digest);
            let Some(user): Option<PasswordUserRecord> = self.get_record(&user_key).await? else {
                self.remove_record(&StoreKey::password_user_by_subject(
                    subject,
                    &index.email_digest,
                ))
                .await?;
                continue;
            };

            let mut operations = vec![TransactOperation::Delete {
                key: StoreKey::password_user_by_subject(subject, &index.email_digest).parts(),
                condition: None,
            }];
            if user.subject.as_deref() == Some(subject) && user.deleted_at.is_none() {
                let mut tombstone = user.clone();
                tombstone.registration_id = None;
                tombstone.email = None;
                tombstone.subject = None;
                tombstone.password_hash = None;
                tombstone.password_hash_updated_at = None;
                tombstone.verified = false;
                tombstone.updated_at = deleted_at;
                tombstone.deleted_at = Some(deleted_at);
                operations.push(TransactOperation::Put {
                    key: user_key.parts(),
                    value: to_value(&tombstone)?,
                    expiry: None,
                    condition: Some(equal(&user)?),
                });
                deleted += 1;
            }
            self.storage.transact(operations).await?;
        }
        Ok(deleted)
    }
}
