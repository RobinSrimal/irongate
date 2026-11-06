//! Proof-bound email ownership and subject-indexed, retryable deletion.

use super::{
    credential_generation::equal,
    keys::StoreKey,
    records::{AccountRecord, AccountStatus, EmailOwnerRecord, EmailSubjectIndexRecord},
    reusable_after, to_value, AuthStore, DeletedIdentityReusePolicy,
};
use crate::{
    core::subjects::Subject,
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(super) enum EmailAccountPlan {
    New {
        digest: String,
        previous: PreviousEmailOwner,
    },
    Existing {
        digest: String,
        owner: Value,
        index: Value,
    },
}

#[derive(Serialize, Deserialize)]
pub(super) enum PreviousEmailOwner {
    Absent,
    Deleted(Value),
}

impl EmailAccountPlan {
    pub(super) fn is_new(&self) -> bool {
        matches!(self, Self::New { .. })
    }

    pub(super) fn operations(&self, subject: &str) -> Result<Vec<TransactOperation>, StorageError> {
        Ok(match self {
            Self::New { digest, previous } => vec![
                TransactOperation::Put {
                    key: StoreKey::email_owner(digest).parts(),
                    value: to_value(&EmailOwnerRecord {
                        email_digest: digest.clone(),
                        subject: Some(subject.into()),
                        deleted_at: None,
                        reusable_after: None,
                    })?,
                    expiry: None,
                    condition: Some(match previous {
                        PreviousEmailOwner::Absent => TransactCondition::NotExists,
                        PreviousEmailOwner::Deleted(value) => equal(value)?,
                    }),
                },
                TransactOperation::Put {
                    key: StoreKey::email_by_subject(subject, digest).parts(),
                    value: to_value(&EmailSubjectIndexRecord {
                        email_digest: digest.clone(),
                        subject: subject.into(),
                    })?,
                    expiry: None,
                    condition: Some(TransactCondition::NotExists),
                },
            ],
            Self::Existing {
                digest,
                owner,
                index,
            } => vec![
                TransactOperation::ConditionCheck {
                    key: StoreKey::email_owner(digest).parts(),
                    condition: equal(&owner)?,
                },
                TransactOperation::ConditionCheck {
                    key: StoreKey::email_by_subject(subject, digest).parts(),
                    condition: equal(&index)?,
                },
            ],
        })
    }

    pub(super) fn validate_snapshot(
        &self,
        digest: &str,
        subject: &str,
    ) -> Result<(), StorageError> {
        let valid = match self {
            Self::New {
                digest: stored,
                previous,
            } => {
                stored == digest
                    && match previous {
                        PreviousEmailOwner::Absent => true,
                        PreviousEmailOwner::Deleted(value) => {
                            let owner: EmailOwnerRecord = serde_json::from_value(value.clone())
                                .map_err(|_| invalid_ownership())?;
                            owner.email_digest == digest
                                && owner.subject.is_none()
                                && owner.deleted_at.is_some()
                                && owner
                                    .reusable_after
                                    .zip(owner.deleted_at)
                                    .is_some_and(|(reuse, deleted)| reuse >= deleted)
                        }
                    }
            }
            Self::Existing {
                digest: stored,
                owner,
                index,
            } => {
                let owner: EmailOwnerRecord =
                    serde_json::from_value(owner.clone()).map_err(|_| invalid_ownership())?;
                let index: EmailSubjectIndexRecord =
                    serde_json::from_value(index.clone()).map_err(|_| invalid_ownership())?;
                stored == digest
                    && owner.email_digest == digest
                    && owner.subject.as_deref() == Some(subject)
                    && owner.deleted_at.is_none()
                    && owner.reusable_after.is_none()
                    && index.subject == subject
                    && index.email_digest == digest
            }
        };
        if valid {
            Ok(())
        } else {
            Err(invalid_ownership())
        }
    }
}

impl AuthStore {
    pub(super) async fn prepare_email_account(
        &self,
        email_digest: &str,
        policy: DeletedIdentityReusePolicy,
        now: DateTime<Utc>,
    ) -> Result<(AccountRecord, EmailAccountPlan), StorageError> {
        let previous = self.read_email_owner(email_digest).await?;
        if let Some((owner, value)) = &previous {
            if let Some(subject) = &owner.subject {
                let account = self
                    .active_credential_account(subject)
                    .await?
                    .ok_or_else(|| {
                        StorageError::ConditionFailed("email account is not active".into())
                    })?;
                let index = self
                    .get_record::<Value>(&StoreKey::email_by_subject(subject, email_digest))
                    .await?
                    .ok_or_else(invalid_ownership)?;
                let binding: EmailSubjectIndexRecord =
                    serde_json::from_value(index.clone()).map_err(|_| invalid_ownership())?;
                if binding.subject != *subject || binding.email_digest != email_digest {
                    return Err(invalid_ownership());
                }
                return Ok((
                    account,
                    EmailAccountPlan::Existing {
                        digest: email_digest.into(),
                        owner: value.clone(),
                        index,
                    },
                ));
            }
            if !owner_reusable(owner, policy, now) {
                return Err(StorageError::AlreadyExists(
                    "email reuse is not permitted".into(),
                ));
            }
        }
        let previous = previous
            .map(|(_, value)| PreviousEmailOwner::Deleted(value))
            .unwrap_or(PreviousEmailOwner::Absent);
        Ok((
            AccountRecord {
                subject: Subject::generate().as_str().into(),
                credential_generation: 0,
                status: AccountStatus::Active,
                created_at: now,
                disabled_at: None,
                deleted_at: None,
            },
            EmailAccountPlan::New {
                digest: email_digest.into(),
                previous,
            },
        ))
    }

    pub(super) async fn prepare_email_ownership(
        &self,
        email_digest: &str,
        policy: DeletedIdentityReusePolicy,
        now: DateTime<Utc>,
    ) -> Result<Option<TransactCondition>, StorageError> {
        let Some((owner, value)) = self.read_email_owner(email_digest).await? else {
            return Ok(Some(TransactCondition::NotExists));
        };
        if !owner_reusable(&owner, policy, now) {
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
        if email_digest.is_empty() {
            return Err(invalid_ownership());
        }
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
        if owner.email_digest != email_digest || !valid_state {
            return Err(invalid_ownership());
        }
        Ok(Some((owner, value)))
    }
}

fn owner_reusable(
    owner: &EmailOwnerRecord,
    policy: DeletedIdentityReusePolicy,
    now: DateTime<Utc>,
) -> bool {
    owner.subject.is_none()
        && policy != DeletedIdentityReusePolicy::Never
        && owner.reusable_after.is_some_and(|deadline| {
            policy != DeletedIdentityReusePolicy::AfterRetention || now >= deadline
        })
}

fn invalid_ownership() -> StorageError {
    StorageError::DynamoDB("invalid email ownership record".into())
}
