//! Password replacement, proof consumption and generation invalidation share one commit.

use super::{
    credential_generation::{equal, guard},
    keys::StoreKey,
    records::{
        AccountRecord, PasswordResetRecord, PasswordResetSubjectIndexRecord, PasswordUserRecord,
    },
    to_value, AuthStore,
};
use crate::{
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};
use chrono::{DateTime, Utc};

/// Validated snapshots are private and consumed by finish, never supplied by callers.
pub struct PreparedPasswordReset {
    digest: String,
    reset: PasswordResetRecord,
    user: PasswordUserRecord,
    account: AccountRecord,
}

fn matches_user(user: &PasswordUserRecord, subject: &str) -> bool {
    user.verified
        && user.deleted_at.is_none()
        && user.password_hash.is_some()
        && user.subject.as_deref() == Some(subject)
}

impl AuthStore {
    pub async fn create_password_reset(
        &self,
        reset_digest: &str,
        email_digest: &str,
        subject: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let account = self
            .active_credential_account(subject)
            .await?
            .ok_or_else(|| StorageError::ConditionFailed("reset account unavailable".into()))?;
        let user = self
            .get_password_user_by_email_digest(email_digest)
            .await?
            .filter(|user| matches_user(user, subject))
            .ok_or_else(|| StorageError::ConditionFailed("reset identity unavailable".into()))?;
        let record = PasswordResetRecord {
            credential_generation: account.credential_generation,
            email_digest: email_digest.into(),
            subject: subject.into(),
            purpose: "reset_password".into(),
            created_at: (self.recovery_now)(),
            expires_at,
        };
        let index = PasswordResetSubjectIndexRecord {
            reset_digest: reset_digest.into(),
            subject: subject.into(),
            expires_at,
        };
        self.storage
            .transact(vec![
                guard(&account)?,
                TransactOperation::ConditionCheck {
                    key: StoreKey::password_user(email_digest).parts(),
                    condition: equal(&user)?,
                },
                TransactOperation::Put {
                    key: StoreKey::password_reset(reset_digest).parts(),
                    value: to_value(&record)?,
                    expiry: Some(expires_at),
                    condition: Some(TransactCondition::NotExists),
                },
                TransactOperation::Put {
                    key: StoreKey::password_reset_by_subject(subject, reset_digest).parts(),
                    value: to_value(&index)?,
                    expiry: Some(expires_at),
                    condition: None,
                },
            ])
            .await
    }

    pub async fn prepare_password_reset(
        &self,
        digest: &str,
    ) -> Result<Option<PreparedPasswordReset>, StorageError> {
        let Some(reset): Option<PasswordResetRecord> =
            self.get_record(&StoreKey::password_reset(digest)).await?
        else {
            return Ok(None);
        };
        if reset.purpose != "reset_password" || (self.recovery_now)() >= reset.expires_at {
            return Ok(None);
        }
        let Some(account) = self
            .account_at_generation(&reset.subject, reset.credential_generation)
            .await?
        else {
            return Ok(None);
        };
        let Some(user) = self
            .get_password_user_by_email_digest(&reset.email_digest)
            .await?
            .filter(|user| matches_user(user, &reset.subject))
        else {
            return Ok(None);
        };
        Ok(Some(PreparedPasswordReset {
            digest: digest.into(),
            reset,
            user,
            account,
        }))
    }

    pub async fn finish_password_reset(
        &self,
        prepared: PreparedPasswordReset,
        hash: &str,
    ) -> Result<bool, StorageError> {
        let PreparedPasswordReset {
            digest,
            reset,
            user,
            account,
        } = prepared;
        let generation = account
            .credential_generation
            .checked_add(1)
            .ok_or_else(|| StorageError::DynamoDB("credential generation exhausted".into()))?;
        let mut changed_account = account.clone();
        changed_account.credential_generation = generation;
        let mut changed_user = user.clone();
        let now = (self.recovery_now)();
        changed_user.password_hash = Some(hash.into());
        changed_user.password_hash_updated_at = Some(now);
        changed_user.updated_at = now;
        let operations = vec![
            TransactOperation::Delete {
                key: StoreKey::password_reset(&digest).parts(),
                condition: Some(equal(&reset)?),
            },
            TransactOperation::Delete {
                key: StoreKey::password_reset_by_subject(&reset.subject, &digest).parts(),
                condition: None,
            },
            TransactOperation::Put {
                key: StoreKey::password_user(&reset.email_digest).parts(),
                value: to_value(&changed_user)?,
                expiry: None,
                condition: Some(equal(&user)?),
            },
            TransactOperation::Put {
                key: StoreKey::account(&reset.subject).parts(),
                value: to_value(&changed_account)?,
                expiry: None,
                condition: Some(equal(&account)?),
            },
        ];
        if (self.recovery_now)() >= reset.expires_at {
            return Ok(false);
        }
        match self.storage.transact(operations).await {
            Ok(()) => Ok(true),
            Err(StorageError::ConditionFailed(_)) => Ok(false),
            Err(error) => Err(error),
        }
    }
}
