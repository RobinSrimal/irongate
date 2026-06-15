//! Refresh-token store operations.

use super::{to_value, AuthStore};
use crate::core::subjects::Subject;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::crypto::random::{generate_random_string, generate_uuid};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{
    RefreshTokenFamilyRecord, RefreshTokenIndexRecord, RefreshTokenRecord,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CreateRefreshTokenInput {
    pub client_id: String,
    pub subject: String,
    pub subject_type: String,
    pub scope: String,
    pub properties: Value,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreatedRefreshToken {
    pub raw_token: String,
    pub refresh_digest: String,
    pub family_id: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RotatedRefreshToken {
    pub raw_token: String,
    pub refresh_digest: String,
    pub family_id: String,
    pub client_id: String,
    pub subject: String,
    pub subject_type: String,
    pub scope: String,
    pub properties: Value,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevokeRefreshTokenOutcome {
    Revoked,
    AlreadyRevoked,
    NotFound,
}

#[derive(Debug, Error)]
pub enum RefreshTokenStoreError {
    #[error("refresh token is invalid")]
    Invalid,

    #[error("refresh token belongs to a different client")]
    WrongClient,

    #[error("refresh token was already used")]
    ReuseDetected,

    #[error("refresh token subject is not active")]
    SubjectInactive,

    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl AuthStore {
    pub async fn create_refresh_token(
        &self,
        lookup_secret: &[u8],
        input: CreateRefreshTokenInput,
    ) -> Result<CreatedRefreshToken, StorageError> {
        let now = Utc::now();
        let raw_token = generate_random_string(64);
        let refresh_digest = lookup_digest(lookup_secret, LookupFamily::RefreshToken, &raw_token);
        let family_id = format!("refresh_family_{}", generate_uuid());

        let record = RefreshTokenRecord {
            refresh_digest: refresh_digest.clone(),
            family_id: family_id.clone(),
            client_id: input.client_id.clone(),
            subject: input.subject.clone(),
            subject_type: input.subject_type.clone(),
            scope: input.scope.clone(),
            properties: input.properties.clone(),
            issued_at: now,
            expires_at: input.expires_at,
            last_used_at: None,
            replaced_by: None,
            revoked_at: None,
        };
        let family = RefreshTokenFamilyRecord {
            family_id: family_id.clone(),
            client_id: input.client_id.clone(),
            subject: input.subject.clone(),
            subject_type: input.subject_type.clone(),
            scope: input.scope.clone(),
            properties: input.properties.clone(),
            current_refresh_digest: refresh_digest.clone(),
            created_at: now,
            expires_at: input.expires_at,
            last_rotated_at: None,
            revoked_at: None,
        };
        let subject_index = RefreshTokenIndexRecord {
            refresh_digest: refresh_digest.clone(),
            family_id: family_id.clone(),
            client_id: input.client_id.clone(),
            subject: input.subject.clone(),
            expires_at: input.expires_at,
        };
        let client_index = subject_index.clone();

        let primary_key = StoreKey::refresh_token(&refresh_digest);
        let primary_parts = primary_key.parts();
        let primary_refs: Vec<&str> = primary_parts.iter().map(String::as_str).collect();
        let inserted = self
            .storage
            .compare_and_set(
                &primary_refs,
                None,
                to_value(&record)?,
                Some(input.expires_at),
            )
            .await?;
        if !inserted {
            return Err(StorageError::AlreadyExists(
                "refresh token digest already exists".into(),
            ));
        }

        self.set_record(
            &StoreKey::refresh_family(&family_id),
            &family,
            Some(input.expires_at),
        )
        .await?;
        self.set_record(
            &StoreKey::refresh_by_subject(&input.subject, &refresh_digest),
            &subject_index,
            Some(input.expires_at),
        )
        .await?;
        self.set_record(
            &StoreKey::refresh_by_client(&input.client_id, &refresh_digest),
            &client_index,
            Some(input.expires_at),
        )
        .await?;

        Ok(CreatedRefreshToken {
            raw_token,
            refresh_digest,
            family_id,
            expires_at: input.expires_at,
        })
    }

    pub async fn get_refresh_token(
        &self,
        refresh_digest: &str,
    ) -> Result<Option<RefreshTokenRecord>, StorageError> {
        self.get_record(&StoreKey::refresh_token(refresh_digest))
            .await
    }

    pub async fn rotate_refresh_token(
        &self,
        lookup_secret: &[u8],
        raw_token: &str,
        client_id: &str,
        new_expires_at: DateTime<Utc>,
    ) -> Result<RotatedRefreshToken, RefreshTokenStoreError> {
        let now = Utc::now();
        let refresh_digest = lookup_digest(lookup_secret, LookupFamily::RefreshToken, raw_token);
        let key = StoreKey::refresh_token(&refresh_digest);
        let record: RefreshTokenRecord = self
            .get_record(&key)
            .await?
            .ok_or(RefreshTokenStoreError::Invalid)?;

        if now >= record.expires_at {
            return Err(RefreshTokenStoreError::Invalid);
        }
        if record.client_id != client_id {
            return Err(RefreshTokenStoreError::WrongClient);
        }

        let family_key = StoreKey::refresh_family(&record.family_id);
        let family: RefreshTokenFamilyRecord = self
            .get_record(&family_key)
            .await?
            .ok_or(RefreshTokenStoreError::Invalid)?;

        if now >= family.expires_at || family.revoked_at.is_some() {
            return Err(RefreshTokenStoreError::Invalid);
        }

        if record.revoked_at.is_some()
            || record.replaced_by.is_some()
            || family.current_refresh_digest != refresh_digest
        {
            self.revoke_refresh_family_by_id(&record.family_id, now)
                .await?;
            return Err(RefreshTokenStoreError::ReuseDetected);
        }

        let subject = Subject::from_persisted(record.subject.clone());
        if !self.is_active_account(&subject).await? {
            return Err(RefreshTokenStoreError::SubjectInactive);
        }

        let new_raw_token = generate_random_string(64);
        let new_digest = lookup_digest(lookup_secret, LookupFamily::RefreshToken, &new_raw_token);

        let mut updated_old = record.clone();
        updated_old.last_used_at = Some(now);
        updated_old.replaced_by = Some(new_digest.clone());

        let new_record = RefreshTokenRecord {
            refresh_digest: new_digest.clone(),
            family_id: record.family_id.clone(),
            client_id: record.client_id.clone(),
            subject: record.subject.clone(),
            subject_type: record.subject_type.clone(),
            scope: record.scope.clone(),
            properties: record.properties.clone(),
            issued_at: now,
            expires_at: new_expires_at,
            last_used_at: None,
            replaced_by: None,
            revoked_at: None,
        };

        let mut updated_family = family.clone();
        updated_family.current_refresh_digest = new_digest.clone();
        updated_family.expires_at = new_expires_at;
        updated_family.last_rotated_at = Some(now);

        let index = RefreshTokenIndexRecord {
            refresh_digest: new_digest.clone(),
            family_id: record.family_id.clone(),
            client_id: record.client_id.clone(),
            subject: record.subject.clone(),
            expires_at: new_expires_at,
        };

        let old_value = to_value(&record)?;
        self.storage
            .transact(vec![
                TransactOperation::Update {
                    key: key.parts(),
                    updates: to_value(&updated_old)?,
                    condition: Some(TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: old_value,
                    }),
                },
                TransactOperation::Update {
                    key: family_key.parts(),
                    updates: to_value(&updated_family)?,
                    condition: Some(TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&family)?,
                    }),
                },
                TransactOperation::Put {
                    key: StoreKey::refresh_token(&new_digest).parts(),
                    value: to_value(&new_record)?,
                    expiry: Some(new_expires_at),
                },
                TransactOperation::Put {
                    key: StoreKey::refresh_by_subject(&record.subject, &new_digest).parts(),
                    value: to_value(&index)?,
                    expiry: Some(new_expires_at),
                },
                TransactOperation::Put {
                    key: StoreKey::refresh_by_client(&record.client_id, &new_digest).parts(),
                    value: to_value(&index)?,
                    expiry: Some(new_expires_at),
                },
            ])
            .await?;

        Ok(RotatedRefreshToken {
            raw_token: new_raw_token,
            refresh_digest: new_digest,
            family_id: record.family_id,
            client_id: record.client_id,
            subject: record.subject,
            subject_type: record.subject_type,
            scope: record.scope,
            properties: record.properties,
            expires_at: new_expires_at,
        })
    }

    pub async fn revoke_refresh_token_family(
        &self,
        lookup_secret: &[u8],
        raw_token: &str,
        client_id: &str,
    ) -> Result<RevokeRefreshTokenOutcome, StorageError> {
        let refresh_digest = lookup_digest(lookup_secret, LookupFamily::RefreshToken, raw_token);
        let record: RefreshTokenRecord = match self
            .get_record(&StoreKey::refresh_token(&refresh_digest))
            .await?
        {
            Some(record) => record,
            None => return Ok(RevokeRefreshTokenOutcome::NotFound),
        };

        if record.client_id != client_id {
            return Ok(RevokeRefreshTokenOutcome::NotFound);
        }

        self.revoke_refresh_family_by_id(&record.family_id, Utc::now())
            .await
    }

    pub async fn revoke_refresh_tokens_for_subject(
        &self,
        subject: &str,
    ) -> Result<usize, StorageError> {
        let subject_pk = StoreKey::refresh_by_subject_pk(subject);
        let rows = self.storage.scan(&[subject_pk.as_str()]).await?;
        let now = Utc::now();
        let mut revoked = 0;

        for (_, value) in rows {
            let index: RefreshTokenIndexRecord = serde_json::from_value(value)
                .map_err(|err| StorageError::DynamoDB(err.to_string()))?;
            match self
                .revoke_refresh_family_by_id(&index.family_id, now)
                .await?
            {
                RevokeRefreshTokenOutcome::Revoked => revoked += 1,
                RevokeRefreshTokenOutcome::AlreadyRevoked | RevokeRefreshTokenOutcome::NotFound => {
                }
            }
        }

        Ok(revoked)
    }

    async fn revoke_refresh_family_by_id(
        &self,
        family_id: &str,
        now: DateTime<Utc>,
    ) -> Result<RevokeRefreshTokenOutcome, StorageError> {
        let family_key = StoreKey::refresh_family(family_id);
        let family: RefreshTokenFamilyRecord = match self.get_record(&family_key).await? {
            Some(family) => family,
            None => return Ok(RevokeRefreshTokenOutcome::NotFound),
        };

        if family.revoked_at.is_some() {
            return Ok(RevokeRefreshTokenOutcome::AlreadyRevoked);
        }

        let mut updated_family = family.clone();
        updated_family.revoked_at = Some(now);

        let current_key = StoreKey::refresh_token(&family.current_refresh_digest);
        let current_record: Option<RefreshTokenRecord> = self.get_record(&current_key).await?;
        let mut operations = vec![TransactOperation::Update {
            key: family_key.parts(),
            updates: to_value(&updated_family)?,
            condition: Some(TransactCondition::AttributeEquals {
                name: "value".to_string(),
                value: to_value(&family)?,
            }),
        }];

        if let Some(current) = current_record {
            if current.revoked_at.is_none() {
                let mut revoked_current = current.clone();
                revoked_current.revoked_at = Some(now);
                operations.push(TransactOperation::Update {
                    key: current_key.parts(),
                    updates: to_value(&revoked_current)?,
                    condition: Some(TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&current)?,
                    }),
                });
            }
        }

        self.storage.transact(operations).await?;
        Ok(RevokeRefreshTokenOutcome::Revoked)
    }
}
