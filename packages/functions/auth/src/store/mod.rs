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
pub mod records;
pub mod refresh;

use crate::core::subjects::Subject;
use crate::error::StorageError;
use crate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use chrono::{DateTime, Duration, Utc};
use keys::StoreKey;
use records::{
    AccountRecord, AccountStatus, IdentityRecord, IdentityStatus, IdentitySubjectIndexRecord,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::str::FromStr;
use std::sync::Arc;

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

#[derive(Debug, Clone)]
pub struct DeleteAccountOutcome {
    pub account: AccountRecord,
    pub deleted_identities: usize,
    pub deleted_password_users: usize,
    pub deleted_password_secrets: usize,
    pub revoked_refresh_families: usize,
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

#[derive(Clone)]
pub struct AuthStore {
    storage: Arc<dyn StorageAdapter>,
}

impl AuthStore {
    pub fn new<S>(storage: S) -> Self
    where
        S: StorageAdapter + 'static,
    {
        Self {
            storage: Arc::new(storage),
        }
    }

    pub(crate) fn from_backend(storage: Arc<dyn StorageAdapter>) -> Self {
        Self { storage }
    }

    pub async fn check_rate_limit(
        &self,
        config: &crate::config::RateLimitConfig,
        endpoint: crate::config::Endpoint,
        identifier: &str,
    ) -> Result<(), crate::error::AuthError> {
        crate::ratelimit::middleware::check_rate_limit(
            self.storage.as_ref(),
            config,
            endpoint,
            identifier,
        )
        .await
    }

    pub async fn record_audit_event(&self, event: crate::audit::AuditEvent) -> Result<(), String> {
        crate::audit::record_event(self.storage.as_ref(), event).await
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
            subject: Some(subject.as_str().to_string()),
            status: IdentityStatus::Active,
            created_at: now,
            last_seen_at: now,
            deleted_at: None,
            reusable_after: None,
            properties: Some(properties),
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
                self.create_account_with_identity(provider, identity_digest, properties)
                    .await
            }
            Some(existing) if existing.status == IdentityStatus::Active => {
                let persisted_subject = existing.subject.clone().ok_or_else(|| {
                    StorageError::DynamoDB("active identity missing subject".into())
                })?;
                let subject = Subject::from_persisted(persisted_subject);
                if !self.is_active_account(&subject).await? {
                    return Err(StorageError::ConditionFailed(
                        "identity account is not active".into(),
                    ));
                }

                let mut updated = existing.clone();
                updated.last_seen_at = Utc::now();
                updated.properties = Some(properties);

                self.storage
                    .transact(vec![
                        TransactOperation::Put {
                            key: key.parts(),
                            value: to_value(&updated)?,
                            expiry: None,
                            condition: Some(TransactCondition::AttributeEquals {
                                name: "value".to_string(),
                                value: to_value(&existing)?,
                            }),
                        },
                    ])
                    .await?;

                Ok(subject)
            }
            Some(existing) if existing.status == IdentityStatus::Deleted => {
                self.reuse_deleted_identity(provider, identity_digest, reuse_policy, properties)
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

    pub async fn disable_account(&self, subject: &Subject) -> Result<AccountRecord, StorageError> {
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

    pub async fn delete_account(
        &self,
        subject: &Subject,
        reuse_policy: DeletedIdentityReusePolicy,
        retention_days: u32,
    ) -> Result<DeleteAccountOutcome, StorageError> {
        if reuse_policy == DeletedIdentityReusePolicy::AfterRetention && retention_days == 0 {
            return Err(StorageError::ConditionFailed(
                "deleted identity retention must be positive".into(),
            ));
        }

        let key = StoreKey::account(subject.as_str());
        let account: AccountRecord = self
            .get_record(&key)
            .await?
            .ok_or_else(|| StorageError::NotFound("account not found".into()))?;

        let account = match account.status {
            AccountStatus::Active | AccountStatus::Disabled => {
                let mut deleted = account.clone();
                deleted.status = AccountStatus::Deleted;
                deleted.disabled_at = None;
                deleted.deleted_at = Some(Utc::now());

                self.storage
                    .transact(vec![TransactOperation::Update {
                        key: key.parts(),
                        updates: to_value(&deleted)?,
                        condition: Some(TransactCondition::AttributeEquals {
                            name: "value".to_string(),
                            value: to_value(&account)?,
                        }),
                    }])
                    .await?;

                deleted
            }
            AccountStatus::Deleted => account,
        };

        let deleted_at = account.deleted_at.unwrap_or_else(Utc::now);
        let deleted_identities = self
            .tombstone_identities_for_subject(
                subject.as_str(),
                reuse_policy,
                retention_days,
                deleted_at,
            )
            .await?;
        let deleted_password_users = self
            .tombstone_password_users_for_subject(subject.as_str(), deleted_at)
            .await?;
        let deleted_password_secrets = self
            .delete_password_secrets_for_subject(subject.as_str())
            .await?;
        let revoked_refresh_families = self
            .revoke_refresh_tokens_for_subject(subject.as_str())
            .await?;

        Ok(DeleteAccountOutcome {
            account,
            deleted_identities,
            deleted_password_users,
            deleted_password_secrets,
            revoked_refresh_families,
        })
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
        identity.subject = None;
        identity.properties = None;

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
        self.ensure_deleted_identity_tombstone_reusable(&existing, policy)
            .await?;
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
            subject: Some(subject.as_str().to_string()),
            status: IdentityStatus::Active,
            created_at: now,
            last_seen_at: now,
            deleted_at: None,
            reusable_after: None,
            properties: Some(properties),
        };
        let index = IdentitySubjectIndexRecord {
            provider: provider.as_str().to_string(),
            identity_digest: identity_digest.to_string(),
            subject: subject.as_str().to_string(),
            created_at: now,
        };

        let expected = to_value(&existing)?;
        self.storage
            .transact(vec![
                TransactOperation::Put {
                    key: StoreKey::account(subject.as_str()).parts(),
                    value: to_value(&account)?,
                    expiry: None,
                    condition: Some(TransactCondition::NotExists),
                },
                TransactOperation::Put {
                    key: StoreKey::identity(provider.as_str(), identity_digest).parts(),
                    value: to_value(&replacement)?,
                    expiry: None,
                    condition: Some(TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: expected,
                    }),
                },
                TransactOperation::Put {
                    key: StoreKey::identity_by_subject(
                        subject.as_str(),
                        provider.as_str(),
                        identity_digest,
                    )
                    .parts(),
                    value: to_value(&index)?,
                    expiry: None,
                    condition: None,
                },
            ])
            .await?;

        Ok(subject)
    }

    pub async fn ensure_deleted_identity_reusable(
        &self,
        provider: IdentityProvider,
        identity_digest: &str,
        policy: DeletedIdentityReusePolicy,
    ) -> Result<(), StorageError> {
        let identity_key = StoreKey::identity(provider.as_str(), identity_digest);
        let Some(identity): Option<IdentityRecord> = self.get_record(&identity_key).await? else {
            return Ok(());
        };

        if identity.status == IdentityStatus::Active {
            return Err(StorageError::AlreadyExists(
                "identity is still active".into(),
            ));
        }

        self.ensure_deleted_identity_tombstone_reusable(&identity, policy)
            .await
    }

    async fn ensure_deleted_identity_tombstone_reusable(
        &self,
        identity: &IdentityRecord,
        policy: DeletedIdentityReusePolicy,
    ) -> Result<(), StorageError> {
        if policy == DeletedIdentityReusePolicy::Never {
            return Err(StorageError::AlreadyExists(
                "deleted identity reuse is disabled".into(),
            ));
        }

        match identity.reusable_after {
            Some(_) if policy == DeletedIdentityReusePolicy::Immediate => Ok(()),
            Some(reusable_after) if Utc::now() >= reusable_after => Ok(()),
            Some(_) => Err(StorageError::AlreadyExists(
                "deleted identity is still inside retention window".into(),
            )),
            None => Err(StorageError::AlreadyExists(
                "deleted identity reuse is disabled".into(),
            )),
        }
    }

    async fn tombstone_identities_for_subject(
        &self,
        subject: &str,
        reuse_policy: DeletedIdentityReusePolicy,
        retention_days: u32,
        deleted_at: DateTime<Utc>,
    ) -> Result<usize, StorageError> {
        let index_pk = StoreKey::identity_by_subject_pk(subject);
        let rows = self.storage.query_prefix(&[index_pk.as_str()]).await?;
        let mut deleted = 0;

        for (_, value) in rows {
            let index: IdentitySubjectIndexRecord = serde_json::from_value(value)
                .map_err(|err| StorageError::DynamoDB(err.to_string()))?;
            let identity_key = StoreKey::identity(&index.provider, &index.identity_digest);
            let Some(identity): Option<IdentityRecord> = self.get_record(&identity_key).await?
            else {
                self.remove_record(&StoreKey::identity_by_subject(
                    subject,
                    &index.provider,
                    &index.identity_digest,
                ))
                .await?;
                continue;
            };

            let mut operations = vec![TransactOperation::Delete {
                key: StoreKey::identity_by_subject(
                    subject,
                    &index.provider,
                    &index.identity_digest,
                )
                .parts(),
            }];

            if identity.status == IdentityStatus::Active
                && identity.subject.as_deref() == Some(subject)
            {
                let mut tombstone = identity.clone();
                tombstone.status = IdentityStatus::Deleted;
                tombstone.subject = None;
                tombstone.properties = None;
                tombstone.deleted_at = Some(deleted_at);
                tombstone.reusable_after =
                    reusable_after(reuse_policy, retention_days, deleted_at)?;

                operations.push(TransactOperation::Put {
                    key: identity_key.parts(),
                    value: to_value(&tombstone)?,
                    expiry: None,
                    condition: Some(TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&identity)?,
                    }),
                });
                deleted += 1;
            }

            self.storage.transact(operations).await?;
        }

        Ok(deleted)
    }

    async fn put_new_account_and_identity(
        &self,
        account: &AccountRecord,
        identity: &IdentityRecord,
    ) -> Result<(), StorageError> {
        let subject = identity
            .subject
            .as_deref()
            .ok_or_else(|| StorageError::DynamoDB("active identity missing subject".into()))?;
        let index = IdentitySubjectIndexRecord {
            provider: identity.provider.clone(),
            identity_digest: identity.identity_digest.clone(),
            subject: subject.to_string(),
            created_at: identity.created_at,
        };

        self.storage
            .transact(vec![
                TransactOperation::Put {
                    key: StoreKey::account(&account.subject).parts(),
                    value: to_value(account)?,
                    expiry: None,
                    condition: Some(TransactCondition::NotExists),
                },
                TransactOperation::Put {
                    key: StoreKey::identity(&identity.provider, &identity.identity_digest).parts(),
                    value: to_value(identity)?,
                    expiry: None,
                    condition: Some(TransactCondition::NotExists),
                },
                TransactOperation::Put {
                    key: StoreKey::identity_by_subject(
                        subject,
                        &identity.provider,
                        &identity.identity_digest,
                    )
                    .parts(),
                    value: to_value(&index)?,
                    expiry: None,
                    condition: None,
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

fn reusable_after(
    policy: DeletedIdentityReusePolicy,
    retention_days: u32,
    deleted_at: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, StorageError> {
    match policy {
        DeletedIdentityReusePolicy::Never => Ok(None),
        DeletedIdentityReusePolicy::Immediate => Ok(Some(deleted_at)),
        DeletedIdentityReusePolicy::AfterRetention => {
            let days = i64::from(retention_days);
            if days <= 0 {
                return Err(StorageError::ConditionFailed(
                    "deleted identity retention must be positive".into(),
                ));
            }
            Ok(Some(deleted_at + Duration::days(days)))
        }
    }
}
