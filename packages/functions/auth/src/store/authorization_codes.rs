//! OAuth authorization-code store operations.

use super::{
    credential_generation::{equal, guard},
    to_value, AuthStore,
};
use crate::error::StorageError;
use crate::storage::{TransactCondition, TransactOperation};
use crate::store::keys::StoreKey;
use crate::store::records::{AuthorizationCodeRecord, PasswordUserRecord};
use chrono::Utc;

impl AuthStore {
    pub async fn create_authorization_code(
        &self,
        code_digest: &str,
        record: AuthorizationCodeRecord,
    ) -> Result<(), StorageError> {
        self.write_authorization_code(code_digest, record, None)
            .await
    }

    /// Bind issuance to the same password-user snapshot whose hash was verified.
    pub async fn create_password_authorization_code(
        &self,
        code_digest: &str,
        record: AuthorizationCodeRecord,
        email_digest: &str,
        verified_user: &PasswordUserRecord,
    ) -> Result<(), StorageError> {
        self.write_authorization_code(code_digest, record, Some((email_digest, verified_user)))
            .await
    }

    async fn write_authorization_code(
        &self,
        code_digest: &str,
        record: AuthorizationCodeRecord,
        password: Option<(&str, &PasswordUserRecord)>,
    ) -> Result<(), StorageError> {
        let account = self
            .account_at_generation(&record.subject, record.credential_generation)
            .await?
            .ok_or_else(|| StorageError::ConditionFailed("credential account changed".into()))?;
        let key = StoreKey::authorization_code(code_digest);
        let mut operations = vec![
            guard(&account)?,
            TransactOperation::Put {
                key: key.parts(),
                value: to_value(&record)?,
                expiry: Some(record.expires_at),
                condition: Some(TransactCondition::NotExists),
            },
        ];
        if let Some((email_digest, user)) = password {
            if !user.verified
                || user.deleted_at.is_some()
                || user.password_hash.is_none()
                || user.subject.as_deref() != Some(record.subject.as_str())
            {
                return Err(StorageError::ConditionFailed(
                    "password identity changed".into(),
                ));
            }
            operations.push(TransactOperation::ConditionCheck {
                key: StoreKey::password_user(email_digest).parts(),
                condition: equal(user)?,
            });
        }
        self.storage.transact(operations).await
    }

    pub async fn take_authorization_code(
        &self,
        code_digest: &str,
    ) -> Result<Option<AuthorizationCodeRecord>, StorageError> {
        let record = match self.get_authorization_code(code_digest).await? {
            Some(record) => record,
            None => return Ok(None),
        };

        self.delete_authorization_code_if_current(code_digest, record)
            .await
    }

    pub async fn get_authorization_code(
        &self,
        code_digest: &str,
    ) -> Result<Option<AuthorizationCodeRecord>, StorageError> {
        let key = StoreKey::authorization_code(code_digest);
        let record: AuthorizationCodeRecord = match self.get_record(&key).await? {
            Some(record) => record,
            None => return Ok(None),
        };

        if Utc::now() >= record.expires_at
            || self
                .account_at_generation(&record.subject, record.credential_generation)
                .await?
                .is_none()
        {
            return Ok(None);
        }

        Ok(Some(record))
    }

    pub async fn delete_authorization_code_if_current(
        &self,
        code_digest: &str,
        record: AuthorizationCodeRecord,
    ) -> Result<Option<AuthorizationCodeRecord>, StorageError> {
        let Some(account) = self
            .account_at_generation(&record.subject, record.credential_generation)
            .await?
        else {
            return Ok(None);
        };
        if Utc::now() >= record.expires_at {
            return Ok(None);
        }
        let key = StoreKey::authorization_code(code_digest);
        let result = self
            .storage
            .transact(vec![
                guard(&account)?,
                TransactOperation::Delete {
                    key: key.parts(),
                    condition: Some(TransactCondition::AttributeEquals {
                        name: "value".to_string(),
                        value: to_value(&record)?,
                    }),
                },
            ])
            .await;

        match result {
            Ok(()) => Ok(Some(record)),
            Err(StorageError::ConditionFailed(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
