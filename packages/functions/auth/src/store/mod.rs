//! Typed auth store facade.
//!
//! This layer gives new auth code purpose-specific storage operations while
//! still using the current DynamoDB adapter underneath.

pub mod authorization_codes;
pub mod authorize_sessions;
pub mod keys;
pub mod password_secrets;
pub mod password_users;
pub mod provider_states;
pub mod rate_limits;
pub mod refresh;
pub mod records;

use crate::core::subjects::Subject;
use crate::error::StorageError;
use crate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use chrono::{DateTime, Utc};
use keys::StoreKey;
use records::{AccountRecord, AccountStatus, IdentityRecord, IdentityStatus};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::str::FromStr;

/// Supported identity provider families for persisted identity mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityProvider {
    Password,
    Google,
    Apple,
}

impl IdentityProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Google => "google",
            Self::Apple => "apple",
        }
    }
}

/// Policy for allowing a deleted reusable identity attribute to sign up again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletedIdentityReusePolicy {
    Never,
    Immediate,
    AfterRetention,
}

impl FromStr for DeletedIdentityReusePolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "never" => Ok(Self::Never),
            "immediate" => Ok(Self::Immediate),
            "after_retention" => Ok(Self::AfterRetention),
            other => Err(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthStore<S> {
    storage: S,
}

impl<S> AuthStore<S>
where
    S: StorageAdapter,
{
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    pub async fn create_account_with_identity(
        &self,
        provider: IdentityProvider,
        identity_digest: &str,
        properties: Value,
    ) -> Result<Subject, StorageError> {
        let subject = Subject::generate();
        let now = Utc::now();

        let account = AccountRecord {
            subject: subject.as_str().to_string(),
            status: AccountStatus::Active,
            created_at: now,
            disabled_at: None,
            deleted_at: None,
        };
        let identity = IdentityRecord {
            provider: provider.as_str().to_string(),
            identity_digest: identity_digest.to_string(),
            subject: subject.as_str().to_string(),
            status: IdentityStatus::Active,
            created_at: now,
            last_seen_at: now,
            deleted_at: None,
            reusable_after: None,
            properties,
        };

        self.put_new_account_and_identity(&account, &identity)
            .await?;
        Ok(subject)
    }

    pub async fn resolve_or_create_google_identity(
        &self,
        identity_digest: &str,
        properties: Value,
        reuse_policy: DeletedIdentityReusePolicy,
    ) -> Result<Subject, StorageError> {
        self.resolve_or_create_provider_identity(
            IdentityProvider::Google,
            identity_digest,
            properties,
            reuse_policy,
        )
        .await
    }

    pub async fn resolve_or_create_apple_identity(
        &self,
        identity_digest: &str,
        properties: Value,
        reuse_policy: DeletedIdentityReusePolicy,
    ) -> Result<Subject, StorageError> {
        self.resolve_or_create_provider_identity(
            IdentityProvider::Apple,
            identity_digest,
            properties,
            reuse_policy,
        )
        .await
    }

    async fn resolve_or_create_provider_identity(
        &self,
        provider: IdentityProvider,
        identity_digest: &str,
        properties: Value,
        reuse_policy: DeletedIdentityReusePolicy,
    ) -> Result<Subject, StorageError> {
        let key = StoreKey::identity(provider.as_str(), identity_digest);
        let existing: Option<IdentityRecord> = self.get_record(&key).await?;

        match existing {
            None => {
                self.create_account_with_identity(
                    provider,
                    identity_digest,
                    properties,
                )
                .await
            }
            Some(existing) if existing.status == IdentityStatus::Active => {
                let subject = Subject::from_persisted(existing.subject.clone());
                if !self.is_active_account(&subject).await? {
                    return Err(StorageError::ConditionFailed(
                        "identity account is not active".into(),
                    ));
                }

                let mut updated = existing.clone();
                updated.last_seen_at = Utc::now();
                updated.properties = properties;

                self.storage
                    .transact(vec![
                        TransactOperation::ConditionCheck {
                            key: key.parts(),
                            condition: TransactCondition::AttributeEquals {
                                name: "value".to_string(),
                                value: to_value(&existing)?,
                            },
                        },
                        TransactOperation::Put {
                            key: key.parts(),
                            value: to_value(&updated)?,
                            expiry: None,
                        },
                    ])
                    .await?;

                Ok(subject)
            }
            Some(existing) if existing.status == IdentityStatus::Deleted => {
                self.reuse_deleted_identity(
                    provider,
                    identity_digest,
                    reuse_policy,
                    properties,
                )
                .await
            }
            Some(_) => Err(StorageError::ConditionFailed(
                "unsupported identity state".into(),
            )),
        }
    }

    pub async fn get_account(
        &self,
        subject: &Subject,
    ) -> Result<Option<AccountRecord>, StorageError> {
        let key = StoreKey::account(subject.as_str());
        self.get_record(&key).await
    }

    pub async fn is_active_account(&self, subject: &Subject) -> Result<bool, StorageError> {
        Ok(self
            .get_account(subject)
            .await?
            .map_or(false, |account| account.status == AccountStatus::Active))
    }

    pub async fn disable_account(
        &self,
        subject: &Subject,
    ) -> Result<AccountRecord, StorageError> {
        let key = StoreKey::account(subject.as_str());
        let account: AccountRecord = self
            .get_record(&key)
            .await?
            .ok_or_else(|| StorageError::NotFound("account not found".into()))?;

        match account.status {
            AccountStatus::Active => {
                let mut disabled = account.clone();
                disabled.status = AccountStatus::Disabled;
                disabled.disabled_at = Some(Utc::now());

                self.storage
                    .transact(vec![TransactOperation::Update {
                        key: key.parts(),
                        updates: to_value(&disabled)?,
                        condition: Some(TransactCondition::AttributeEquals {
                            name: "value".to_string(),
                            value: to_value(&account)?,
                        }),
                    }])
                    .await?;

                Ok(disabled)
            }
            AccountStatus::Disabled => Ok(account),
            AccountStatus::Deleted => Err(StorageError::ConditionFailed(
                "deleted account cannot be disabled".into(),
            )),
        }
    }

    pub async fn get_identity(
        &self,
        provider: IdentityProvider,
        identity_digest: &str,
    ) -> Result<Option<IdentityRecord>, StorageError> {
        let key = StoreKey::identity(provider.as_str(), identity_digest);
        self.get_record(&key).await
    }

    pub async fn delete_identity(
        &self,
        provider: IdentityProvider,
        identity_digest: &str,
        reusable_after: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let key = StoreKey::identity(provider.as_str(), identity_digest);
        let mut identity: IdentityRecord = self
            .get_record(&key)
            .await?
            .ok_or_else(|| StorageError::NotFound("identity not found".into()))?;

        identity.status = IdentityStatus::Deleted;
        identity.deleted_at = Some(Utc::now());
        identity.reusable_after = Some(reusable_after);

        self.set_record(&key, &identity, None).await
    }

    pub async fn reuse_deleted_identity(
        &self,
        provider: IdentityProvider,
        identity_digest: &str,
        policy: DeletedIdentityReusePolicy,
        properties: Value,
    ) -> Result<Subject, StorageError> {
        let identity_key = StoreKey::identity(provider.as_str(), identity_digest);
        let existing: IdentityRecord = self
            .get_record(&identity_key)
            .await?
            .ok_or_else(|| StorageError::NotFound("identity not found".into()))?;

        if existing.status != IdentityStatus::Deleted {
            return Err(StorageError::AlreadyExists(
                "identity is still active".into(),
            ));
        }
        if policy == DeletedIdentityReusePolicy::Never {
            return Err(StorageError::AlreadyExists(
                "deleted identity reuse is disabled".into(),
            ));
        }
        if policy == DeletedIdentityReusePolicy::AfterRetention {
            if let Some(reusable_after) = existing.reusable_after {
                if Utc::now() < reusable_after {
                    return Err(StorageError::AlreadyExists(
                        "deleted identity is still inside retention window".into(),
                    ));
                }
            } else {
                return Err(StorageError::AlreadyExists(
                    "deleted identity is missing retention metadata".into(),
                ));
            }
        }

        let subject = Subject::generate();
        let now = Utc::now();
        let account = AccountRecord {
            subject: subject.as_str().to_string(),
            status: AccountStatus::Active,
            created_at: now,
            disabled_at: None,
            deleted_at: None,
        };
        let replacement = IdentityRecord {
            provider: provider.as_str().to_string(),
            identity_digest: identity_digest.to_string(),
            subject: subject.as_str().to_string(),
            status: IdentityStatus::Active,
            created_at: now,
            last_seen_at: now,
            deleted_at: None,
            reusable_after: None,
            properties,
        };

        let expected = to_value(&existing)?;
        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: StoreKey::account(subject.as_str()).parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::ConditionCheck {
                    key: identity_key.parts(),
                    condition: TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: expected,
                    },
                },
                TransactOperation::Put {
                    key: StoreKey::account(subject.as_str()).parts(),
                    value: to_value(&account)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: StoreKey::identity(provider.as_str(), identity_digest).parts(),
                    value: to_value(&replacement)?,
                    expiry: None,
                },
            ])
            .await?;

        Ok(subject)
    }

    async fn put_new_account_and_identity(
        &self,
        account: &AccountRecord,
        identity: &IdentityRecord,
    ) -> Result<(), StorageError> {
        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: StoreKey::account(&account.subject).parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::ConditionCheck {
                    key: StoreKey::identity(&identity.provider, &identity.identity_digest).parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::Put {
                    key: StoreKey::account(&account.subject).parts(),
                    value: to_value(account)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: StoreKey::identity(&identity.provider, &identity.identity_digest).parts(),
                    value: to_value(identity)?,
                    expiry: None,
                },
            ])
            .await
    }

    async fn get_record<T>(&self, key: &StoreKey) -> Result<Option<T>, StorageError>
    where
        T: DeserializeOwned,
    {
        let parts = key.parts();
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        self.storage
            .get(&refs)
            .await?
            .map(serde_json::from_value)
            .transpose()
            .map_err(|err| StorageError::DynamoDB(err.to_string()))
    }

    async fn set_record<T>(
        &self,
        key: &StoreKey,
        record: &T,
        expiry: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError>
    where
        T: Serialize,
    {
        let parts = key.parts();
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        self.storage.set(&refs, to_value(record)?, expiry).await
    }

    async fn remove_record(&self, key: &StoreKey) -> Result<(), StorageError> {
        let parts = key.parts();
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        self.storage.remove(&refs).await
    }
}

fn to_value<T: Serialize>(value: &T) -> Result<Value, StorageError> {
    serde_json::to_value(value).map_err(|err| StorageError::DynamoDB(err.to_string()))
}
