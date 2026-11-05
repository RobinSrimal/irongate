//! Proof-bound email ownership and subject-indexed, retryable deletion.

use super::{
    credential_generation::equal,
    keys::StoreKey,
    records::{EmailOwnerRecord, EmailSubjectIndexRecord},
    reusable_after, to_value, AuthStore, DeletedIdentityReusePolicy,
};
use crate::{
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};
use chrono::{DateTime, Utc};
use serde_json::Value;

impl AuthStore {
    pub(super) async fn prepare_email_ownership(
        &self,
        email_digest: &str,
        policy: DeletedIdentityReusePolicy,
        now: DateTime<Utc>,
    ) -> Result<Option<TransactCondition>, StorageError> {
        let Some((owner, value)) = self.read_email_owner(email_digest).await? else {
            return Ok(Some(TransactCondition::NotExists));
        };
        if owner.subject.is_some()
            || policy == DeletedIdentityReusePolicy::Never
            || owner.reusable_after.is_none()
            || (policy == DeletedIdentityReusePolicy::AfterRetention
                && owner.reusable_after.is_some_and(|deadline| now < deadline))
        {
            return Ok(None);
        }
        Ok(Some(equal(&value)?))
    }

    pub(super) async fn tombstone_email_owners(
        &self,
        subject: &str,
        policy: DeletedIdentityReusePolicy,
        retention_days: u32,
        deleted_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let pk = StoreKey::email_by_subject_pk(subject);
        for (key, value) in self.storage.query_prefix(&[&pk]).await? {
            let index: EmailSubjectIndexRecord =
                serde_json::from_value(value.clone()).map_err(|_| invalid_ownership())?;
            let index_key = StoreKey::email_by_subject(subject, &index.email_digest);
            if index.subject != subject || index.email_digest.is_empty() || key != index_key.parts()
            {
                return Err(invalid_ownership());
            }

            let owner_key = StoreKey::email_owner(&index.email_digest);
            let mut operations = vec![TransactOperation::Delete {
                key: index_key.parts(),
                condition: Some(equal(&value)?),
            }];
            match self.read_email_owner(&index.email_digest).await? {
                Some((owner, previous)) if owner.subject.as_deref() == Some(subject) => {
                    let tombstone = EmailOwnerRecord {
                        email_digest: index.email_digest,
                        subject: None,
                        deleted_at: Some(deleted_at),
                        reusable_after: reusable_after(policy, retention_days, deleted_at)?,
                    };
                    operations.push(TransactOperation::Put {
                        key: owner_key.parts(),
                        value: to_value(&tombstone)?,
                        expiry: None,
                        condition: Some(equal(&previous)?),
                    });
                }
                Some((_, previous)) => operations.push(TransactOperation::ConditionCheck {
                    key: owner_key.parts(),
                    condition: equal(&previous)?,
                }),
                None => operations.push(TransactOperation::ConditionCheck {
                    key: owner_key.parts(),
                    condition: TransactCondition::NotExists,
                }),
            }
            // Keep the index until its guarded owner transition (or stale-index check) commits.
            self.storage.transact(operations).await?;
        }
        Ok(())
    }

    async fn read_email_owner(
        &self,
        email_digest: &str,
    ) -> Result<Option<(EmailOwnerRecord, Value)>, StorageError> {
        let Some(value) = self
            .get_record::<Value>(&StoreKey::email_owner(email_digest))
            .await?
        else {
            return Ok(None);
        };
        let owner: EmailOwnerRecord =
            serde_json::from_value(value.clone()).map_err(|_| invalid_ownership())?;
        let valid_state = match (&owner.subject, owner.deleted_at) {
            (Some(subject), None) => !subject.trim().is_empty() && owner.reusable_after.is_none(),
            (None, Some(deleted_at)) => owner
                .reusable_after
                .is_none_or(|deadline| deadline >= deleted_at),
            _ => false,
        };
        if email_digest.is_empty() || owner.email_digest != email_digest || !valid_state {
            return Err(invalid_ownership());
        }
        Ok(Some((owner, value)))
    }
}

fn invalid_ownership() -> StorageError {
    StorageError::DynamoDB("invalid email ownership record".into())
}
