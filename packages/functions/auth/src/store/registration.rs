//! Email proof consumption and initial password activation share one commit.

use super::{
    credential_generation::equal,
    keys::StoreKey,
    records::{
        AccountRecord, AccountStatus, EmailOwnerRecord, EmailSubjectIndexRecord,
        EmailVerificationRecord, IdentityRecord, IdentityStatus, IdentitySubjectIndexRecord,
        PasswordUserRecord, PasswordUserSubjectIndexRecord,
    },
    to_value, AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
};
use crate::{
    core::subjects::Subject,
    crypto::hmac_lookup::{lookup_digest, LookupFamily},
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};

/// Only preparation can construct these validated snapshots; finishing consumes them.
pub struct PreparedPasswordRegistration {
    digest: String,
    proof: EmailVerificationRecord,
    user: PasswordUserRecord,
    identity_digest: String,
    identity_condition: TransactCondition,
    email_condition: TransactCondition,
}

impl AuthStore {
    pub async fn prepare_password_registration(
        &self,
        digest: &str,
        lookup_secret: &[u8],
        reuse_policy: DeletedIdentityReusePolicy,
    ) -> Result<Option<PreparedPasswordRegistration>, StorageError> {
        let Some(value): Option<serde_json::Value> = self
            .get_record(&StoreKey::password_verification(digest))
            .await?
        else {
            return Ok(None);
        };
        // Unbound proofs cannot authorize registration, including pre-contract records.
        if value
            .get("registration_id")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Ok(None);
        }
        let proof: EmailVerificationRecord = serde_json::from_value(value)
            .map_err(|_| StorageError::DynamoDB("invalid verification record".into()))?;
        if proof.purpose != "verify_email" || (self.registration_now)() >= proof.expires_at {
            return Ok(None);
        }
        let Some(user) = self
            .get_password_user_by_email_digest(&proof.email_digest)
            .await?
            .filter(|user| {
                user.is_pending_registration()
                    && user.registration_id.as_deref() == Some(&proof.registration_id)
            })
        else {
            return Ok(None);
        };
        let email = user.email.as_deref().expect("validated pending email");
        if lookup_digest(lookup_secret, LookupFamily::Email, email) != proof.email_digest {
            return Ok(None);
        }
        let identity_digest = lookup_digest(lookup_secret, LookupFamily::PasswordIdentity, email);
        let identity_key =
            StoreKey::identity(IdentityProvider::Password.as_str(), &identity_digest);
        let identity: Option<IdentityRecord> = self.get_record(&identity_key).await?;
        let identity_condition = match identity {
            None => TransactCondition::NotExists,
            Some(identity) if identity.status == IdentityStatus::Deleted => {
                match self
                    .ensure_deleted_identity_tombstone_reusable(&identity, reuse_policy)
                    .await
                {
                    Ok(()) => equal(&identity)?,
                    Err(StorageError::AlreadyExists(_)) => return Ok(None),
                    Err(error) => return Err(error),
                }
            }
            Some(_) => return Ok(None),
        };
        let Some(email_condition) = self
            .prepare_email_ownership(&proof.email_digest, reuse_policy, (self.registration_now)())
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(PreparedPasswordRegistration {
            digest: digest.into(),
            proof,
            user,
            identity_digest,
            identity_condition,
            email_condition,
        }))
    }

    pub async fn finish_password_registration(
        &self,
        prepared: PreparedPasswordRegistration,
        hash: &str,
    ) -> Result<Option<Subject>, StorageError> {
        let PreparedPasswordRegistration {
            digest,
            proof,
            user,
            identity_digest,
            identity_condition,
            email_condition,
        } = prepared;
        let subject = Subject::generate();
        let now = (self.registration_now)();
        if now >= proof.expires_at
            || !user.is_pending_registration()
            || user.registration_id.as_deref() != Some(&proof.registration_id)
        {
            return Ok(None);
        }
        let account = AccountRecord {
            credential_generation: 0,
            subject: subject.as_str().into(),
            status: AccountStatus::Active,
            created_at: now,
            disabled_at: None,
            deleted_at: None,
        };
        let provider = IdentityProvider::Password.as_str();
        let identity = IdentityRecord {
            provider: provider.into(),
            identity_digest: identity_digest.clone(),
            subject: Some(subject.as_str().into()),
            status: IdentityStatus::Active,
            created_at: now,
            last_seen_at: now,
            deleted_at: None,
            reusable_after: None,
            properties: Some(serde_json::json!({"email": user.email, "email_verified": true})),
        };
        let mut verified = user.clone();
        verified.registration_id = None;
        verified.subject = Some(subject.as_str().into());
        verified.password_hash = Some(hash.into());
        verified.password_hash_updated_at = Some(now);
        verified.verified = true;
        verified.updated_at = now;
        let identity_index = IdentitySubjectIndexRecord {
            provider: provider.into(),
            identity_digest: identity_digest.clone(),
            subject: subject.as_str().into(),
            created_at: now,
        };
        let password_index = PasswordUserSubjectIndexRecord {
            email_digest: proof.email_digest.clone(),
            subject: subject.as_str().into(),
            created_at: now,
        };
        let email_owner = EmailOwnerRecord {
            email_digest: proof.email_digest.clone(),
            subject: Some(subject.as_str().into()),
            deleted_at: None,
            reusable_after: None,
        };
        let email_index = EmailSubjectIndexRecord {
            email_digest: proof.email_digest.clone(),
            subject: subject.as_str().into(),
        };
        let operations = vec![
            TransactOperation::Delete {
                key: StoreKey::password_verification(&digest).parts(),
                condition: Some(equal(&proof)?),
            },
            TransactOperation::Put {
                key: StoreKey::password_user(&proof.email_digest).parts(),
                value: to_value(&verified)?,
                expiry: None,
                condition: Some(equal(&user)?),
            },
            TransactOperation::Put {
                key: StoreKey::account(subject.as_str()).parts(),
                value: to_value(&account)?,
                expiry: None,
                condition: Some(TransactCondition::NotExists),
            },
            TransactOperation::Put {
                key: StoreKey::identity(provider, &identity_digest).parts(),
                value: to_value(&identity)?,
                expiry: None,
                condition: Some(identity_condition),
            },
            TransactOperation::Put {
                key: StoreKey::identity_by_subject(subject.as_str(), provider, &identity_digest)
                    .parts(),
                value: to_value(&identity_index)?,
                expiry: None,
                condition: Some(TransactCondition::NotExists),
            },
            TransactOperation::Put {
                key: StoreKey::password_user_by_subject(subject.as_str(), &proof.email_digest)
                    .parts(),
                value: to_value(&password_index)?,
                expiry: None,
                condition: Some(TransactCondition::NotExists),
            },
            TransactOperation::Put {
                key: StoreKey::email_owner(&proof.email_digest).parts(),
                value: to_value(&email_owner)?,
                expiry: None,
                condition: Some(email_condition),
            },
            TransactOperation::Put {
                key: StoreKey::email_by_subject(subject.as_str(), &proof.email_digest).parts(),
                value: to_value(&email_index)?,
                expiry: None,
                condition: Some(TransactCondition::NotExists),
            },
        ];
        if (self.registration_now)() >= proof.expires_at {
            return Ok(None);
        }
        match self.storage.transact(operations).await {
            Ok(()) => Ok(Some(subject)),
            Err(StorageError::ConditionFailed(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
