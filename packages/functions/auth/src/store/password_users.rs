//! Password user store operations.

use super::{to_value, AuthStore, IdentityProvider};
use crate::core::subjects::Subject;
use crate::error::StorageError;
use crate::storage::{StorageAdapter, TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{
    AccountRecord, AccountStatus, IdentityRecord, IdentityStatus, IdentitySubjectIndexRecord,
    PasswordUserRecord, PasswordUserSubjectIndexRecord,
};
use chrono::Utc;
use serde_json::Value;

impl<S> AuthStore<S>
where
    S: StorageAdapter,
{
    pub async fn create_unverified_password_user(
        &self,
        email_digest: &str,
        email: &str,
        password_hash: &str,
    ) -> Result<(), StorageError> {
        let now = Utc::now();
        let record = PasswordUserRecord {
            email: Some(email.to_string()),
            subject: None,
            password_hash: Some(password_hash.to_string()),
            password_hash_updated_at: Some(now),
            verified: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        let key = StoreKey::password_user(email_digest);

        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: key.parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::Put {
                    key: key.parts(),
                    value: to_value(&record)?,
                    expiry: None,
                },
            ])
            .await
    }

    pub async fn get_password_user_by_email_digest(
        &self,
        email_digest: &str,
    ) -> Result<Option<PasswordUserRecord>, StorageError> {
        let key = StoreKey::password_user(email_digest);
        self.get_record(&key).await
    }

    pub async fn mark_password_user_verified(
        &self,
        email_digest: &str,
        subject: &Subject,
    ) -> Result<(), StorageError> {
        let key = StoreKey::password_user(email_digest);
        let existing: PasswordUserRecord = self
            .get_record(&key)
            .await?
            .ok_or_else(|| StorageError::NotFound("password user not found".into()))?;

        let mut verified = existing.clone();
        verified.subject = Some(subject.as_str().to_string());
        verified.verified = true;
        verified.updated_at = Utc::now();
        let index = PasswordUserSubjectIndexRecord {
            email_digest: email_digest.to_string(),
            subject: subject.as_str().to_string(),
            created_at: verified.updated_at,
        };

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
                    value: to_value(&verified)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: StoreKey::password_user_by_subject(subject.as_str(), email_digest).parts(),
                    value: to_value(&index)?,
                    expiry: None,
                },
            ])
            .await
    }

    pub async fn verify_password_user_with_identity(
        &self,
        email_digest: &str,
        provider: IdentityProvider,
        identity_digest: &str,
        properties: Value,
    ) -> Result<Subject, StorageError> {
        let password_key = StoreKey::password_user(email_digest);
        let existing: PasswordUserRecord = self
            .get_record(&password_key)
            .await?
            .ok_or_else(|| StorageError::NotFound("password user not found".into()))?;

        if existing.verified {
            return existing
                .subject
                .map(Subject::from_persisted)
                .ok_or_else(|| {
                    StorageError::DynamoDB("verified password user missing subject".into())
                });
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
        let mut verified_user = existing.clone();
        verified_user.subject = Some(subject.as_str().to_string());
        verified_user.verified = true;
        verified_user.updated_at = now;

        let account_key = StoreKey::account(subject.as_str());
        let identity_key = StoreKey::identity(provider.as_str(), identity_digest);
        let identity_index = IdentitySubjectIndexRecord {
            provider: provider.as_str().to_string(),
            identity_digest: identity_digest.to_string(),
            subject: subject.as_str().to_string(),
            created_at: now,
        };
        let password_index = PasswordUserSubjectIndexRecord {
            email_digest: email_digest.to_string(),
            subject: subject.as_str().to_string(),
            created_at: now,
        };

        self.storage
            .transact(vec![
                TransactOperation::ConditionCheck {
                    key: password_key.parts(),
                    condition: TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&existing)?,
                    },
                },
                TransactOperation::ConditionCheck {
                    key: account_key.parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::ConditionCheck {
                    key: identity_key.parts(),
                    condition: TransactCondition::NotExists,
                },
                TransactOperation::Put {
                    key: account_key.parts(),
                    value: to_value(&account)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: identity_key.parts(),
                    value: to_value(&identity)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: StoreKey::identity_by_subject(
                        subject.as_str(),
                        provider.as_str(),
                        identity_digest,
                    )
                    .parts(),
                    value: to_value(&identity_index)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: password_key.parts(),
                    value: to_value(&verified_user)?,
                    expiry: None,
                },
                TransactOperation::Put {
                    key: StoreKey::password_user_by_subject(subject.as_str(), email_digest).parts(),
                    value: to_value(&password_index)?,
                    expiry: None,
                },
            ])
            .await?;

        Ok(subject)
    }

    pub async fn update_password_hash(
        &self,
        email_digest: &str,
        expected_subject: &str,
        password_hash: &str,
    ) -> Result<(), StorageError> {
        let key = StoreKey::password_user(email_digest);
        let existing: PasswordUserRecord = self
            .get_record(&key)
            .await?
            .ok_or_else(|| StorageError::NotFound("password user not found".into()))?;

        if !existing.verified {
            return Err(StorageError::ConditionFailed(
                "password user is not verified".into(),
            ));
        }
        if existing.deleted_at.is_some() {
            return Err(StorageError::ConditionFailed(
                "password user is deleted".into(),
            ));
        }
        if existing.subject.as_deref() != Some(expected_subject) {
            return Err(StorageError::ConditionFailed(
                "password user subject mismatch".into(),
            ));
        }

        let now = Utc::now();
        let mut updated = existing.clone();
        updated.password_hash = Some(password_hash.to_string());
        updated.password_hash_updated_at = Some(now);
        updated.updated_at = now;

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
            .await
    }

    pub async fn tombstone_password_users_for_subject(
        &self,
        subject: &str,
        deleted_at: chrono::DateTime<Utc>,
    ) -> Result<usize, StorageError> {
        let index_pk = StoreKey::password_user_by_subject_pk(subject);
        let rows = self.storage.scan(&[index_pk.as_str()]).await?;
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
            }];

            if user.subject.as_deref() == Some(subject) && user.deleted_at.is_none() {
                let mut tombstone = user.clone();
                tombstone.email = None;
                tombstone.subject = None;
                tombstone.password_hash = None;
                tombstone.password_hash_updated_at = None;
                tombstone.verified = false;
                tombstone.updated_at = deleted_at;
                tombstone.deleted_at = Some(deleted_at);

                operations.push(TransactOperation::ConditionCheck {
                    key: user_key.parts(),
                    condition: TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&user)?,
                    },
                });
                operations.push(TransactOperation::Put {
                    key: user_key.parts(),
                    value: to_value(&tombstone)?,
                    expiry: None,
                });
                deleted += 1;
            }

            self.storage.transact(operations).await?;
        }

        Ok(deleted)
    }
}
