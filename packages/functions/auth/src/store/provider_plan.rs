//! Private new-identity snapshots shared by direct issuance and durable confirmation.

use super::{
    credential_generation::{equal, guard},
    email_ownership::EmailAccountPlan,
    keys::StoreKey,
    records::{
        AccountRecord, AccountStatus, IdentityRecord, IdentityStatus, IdentitySubjectIndexRecord,
    },
    to_value, AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
};
use crate::{
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
enum PreviousIdentity {
    Absent,
    Deleted(IdentityRecord),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewProviderPlan {
    provider: String,
    identity_digest: String,
    previous: PreviousIdentity,
    pub(super) account: AccountRecord,
    email: EmailAccountPlan,
    created_at: DateTime<Utc>,
}

impl NewProviderPlan {
    pub(super) fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub(super) fn identity(&self, properties: Value, now: DateTime<Utc>) -> IdentityRecord {
        IdentityRecord {
            provider: self.provider.clone(),
            identity_digest: self.identity_digest.clone(),
            subject: Some(self.account.subject.clone()),
            status: IdentityStatus::Active,
            created_at: self.created_at,
            last_seen_at: now,
            deleted_at: None,
            reusable_after: None,
            properties: Some(properties),
        }
    }

    pub(super) fn condition(&self) -> Result<TransactCondition, StorageError> {
        match &self.previous {
            PreviousIdentity::Absent => Ok(TransactCondition::NotExists),
            PreviousIdentity::Deleted(identity) => equal(identity),
        }
    }

    pub(super) fn operations(
        &self,
        properties: Value,
        now: DateTime<Utc>,
    ) -> Result<Vec<TransactOperation>, StorageError> {
        let account = &self.account;
        let identity = self.identity(properties, now);
        let mut operations = vec![if self.email.is_new() {
            TransactOperation::Put {
                key: StoreKey::account(&account.subject).parts(),
                value: to_value(account)?,
                expiry: None,
                condition: Some(TransactCondition::NotExists),
            }
        } else {
            guard(account)?
        }];
        operations.push(TransactOperation::Put {
            key: StoreKey::identity_by_subject(
                &account.subject,
                &self.provider,
                &self.identity_digest,
            )
            .parts(),
            value: to_value(&IdentitySubjectIndexRecord {
                provider: self.provider.clone(),
                identity_digest: self.identity_digest.clone(),
                subject: account.subject.clone(),
                created_at: self.created_at,
            })?,
            expiry: None,
            condition: Some(TransactCondition::NotExists),
        });
        operations.extend(self.email.operations(&account.subject)?);
        operations.push(TransactOperation::Put {
            key: StoreKey::identity(&self.provider, &self.identity_digest).parts(),
            value: to_value(&identity)?,
            expiry: None,
            condition: Some(self.condition()?),
        });
        Ok(operations)
    }

    pub(super) fn creation_guards(&self) -> Result<Vec<TransactOperation>, StorageError> {
        // All five issuance items already have exact/absence conditions; creation writes none of them.
        self.operations(Value::Null, self.created_at)?
            .into_iter()
            .map(|operation| match operation {
                TransactOperation::Put {
                    key,
                    condition: Some(condition),
                    ..
                } => Ok(TransactOperation::ConditionCheck { key, condition }),
                check @ TransactOperation::ConditionCheck { .. } => Ok(check),
                _ => Err(invalid_plan()),
            })
            .collect()
    }

    pub(super) fn validate_google_snapshot(
        &self,
        identity_digest: &str,
        email_digest: &str,
        created_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let account = &self.account;
        if self.provider != "google"
            || self.identity_digest != identity_digest
            || self.created_at != created_at
            || account.subject.trim().is_empty()
            || account.status != AccountStatus::Active
            || account.disabled_at.is_some()
            || account.deleted_at.is_some()
            || (self.email.is_new()
                && (account.credential_generation != 0 || account.created_at != created_at))
        {
            return Err(invalid_plan());
        }
        if let PreviousIdentity::Deleted(identity) = &self.previous {
            if identity.provider != self.provider
                || identity.identity_digest != identity_digest
                || identity.status != IdentityStatus::Deleted
                || identity.subject.is_some()
                || identity.properties.is_some()
                || identity.deleted_at.is_none()
                || identity.reusable_after.is_none()
            {
                return Err(invalid_plan());
            }
        }
        self.email.validate_snapshot(email_digest, &account.subject)
    }
}

impl AuthStore {
    pub(super) async fn read_provider_identity(
        &self,
        provider: IdentityProvider,
        digest: &str,
    ) -> Result<Option<IdentityRecord>, StorageError> {
        let identity: Option<IdentityRecord> = self
            .get_record(&StoreKey::identity(provider.as_str(), digest))
            .await?;
        if identity.as_ref().is_some_and(|value| {
            value.provider != provider.as_str() || value.identity_digest != digest
        }) {
            return Err(StorageError::ConditionFailed(
                "provider identity binding changed".into(),
            ));
        }
        Ok(identity)
    }

    pub(super) async fn prepare_new_provider_plan(
        &self,
        provider: IdentityProvider,
        digest: &str,
        existing: Option<IdentityRecord>,
        email_digest: &str,
        policy: DeletedIdentityReusePolicy,
    ) -> Result<NewProviderPlan, StorageError> {
        let previous = match existing {
            Some(identity) => {
                if identity.status != IdentityStatus::Deleted {
                    return Err(StorageError::ConditionFailed(
                        "provider identity already active".into(),
                    ));
                }
                self.ensure_deleted_identity_tombstone_reusable(&identity, policy)
                    .await?;
                PreviousIdentity::Deleted(identity)
            }
            None => PreviousIdentity::Absent,
        };
        let now = (self.provider_issuance_now)();
        let (account, email) = self
            .prepare_email_account(email_digest, policy, now)
            .await?;
        Ok(NewProviderPlan {
            provider: provider.as_str().into(),
            identity_digest: digest.into(),
            previous,
            account,
            email,
            created_at: now,
        })
    }
}

fn invalid_plan() -> StorageError {
    StorageError::DynamoDB("invalid provider resolution snapshot".into())
}
