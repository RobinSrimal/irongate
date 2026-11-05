//! Provider identity updates and authorization-code issuance share one commit.

use super::{
    credential_generation::{equal, guard},
    keys::StoreKey,
    records::{
        AccountRecord, AccountStatus, AuthorizationCodeRecord, AuthorizeSessionRecord,
        IdentityRecord, IdentityStatus, IdentitySubjectIndexRecord,
    },
    to_value, AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
};
use crate::{
    core::subjects::Subject,
    error::StorageError,
    storage::{TransactCondition, TransactOperation},
};
use chrono::Duration;
use serde_json::{json, Value};

/// Supplied by the callback only after full provider-token validation.
pub struct ProviderIdentityInput<'a> {
    pub provider: IdentityProvider,
    pub identity_digest: &'a str,
    pub properties: Value,
}

/// Private snapshots cannot be replaced by a caller between preparation and commit.
pub struct PreparedProviderIssuance {
    account: AccountRecord,
    identity: IdentityRecord,
    identity_condition: TransactCondition,
    new_account: bool,
    session: AuthorizeSessionRecord,
    code_ttl: Duration,
}

impl AuthStore {
    pub async fn prepare_provider_issuance(
        &self,
        input: ProviderIdentityInput<'_>,
        session: &AuthorizeSessionRecord,
        code_ttl_seconds: u64,
        reuse_policy: DeletedIdentityReusePolicy,
    ) -> Result<PreparedProviderIssuance, StorageError> {
        let now = (self.provider_issuance_now)();
        if !matches!(
            input.provider,
            IdentityProvider::Google | IdentityProvider::Apple
        ) || input.identity_digest.is_empty()
            || session.selected_provider.as_deref() != Some(input.provider.as_str())
            || now >= session.expires_at
        {
            return Err(stale_sign_in());
        }
        let code_ttl = i64::try_from(code_ttl_seconds)
            .ok()
            .filter(|seconds| *seconds > 0)
            .and_then(Duration::try_seconds)
            .ok_or_else(|| StorageError::DynamoDB("invalid authorization code TTL".into()))?;
        let key = StoreKey::identity(input.provider.as_str(), input.identity_digest);
        let existing: Option<IdentityRecord> = self.get_record(&key).await?;
        if existing.as_ref().is_some_and(|identity| {
            identity.provider != input.provider.as_str()
                || identity.identity_digest != input.identity_digest
        }) {
            return Err(stale_sign_in());
        }

        let (account, identity, identity_condition, new_account) = match existing {
            Some(identity) if identity.status == IdentityStatus::Active => {
                let subject = identity
                    .subject
                    .as_deref()
                    .filter(|subject| !subject.trim().is_empty())
                    .ok_or_else(stale_sign_in)?;
                let account = self
                    .active_credential_account(subject)
                    .await?
                    .ok_or_else(stale_sign_in)?;
                let condition = equal(&identity)?;
                (account, identity, condition, false)
            }
            existing => {
                let condition = match existing {
                    Some(identity) => {
                        self.ensure_deleted_identity_tombstone_reusable(&identity, reuse_policy)
                            .await?;
                        equal(&identity)?
                    }
                    None => TransactCondition::NotExists,
                };
                let subject = Subject::generate();
                let account = AccountRecord {
                    credential_generation: 0,
                    subject: subject.as_str().into(),
                    status: AccountStatus::Active,
                    created_at: now,
                    disabled_at: None,
                    deleted_at: None,
                };
                let identity = IdentityRecord {
                    provider: input.provider.as_str().into(),
                    identity_digest: input.identity_digest.into(),
                    subject: Some(subject.as_str().into()),
                    status: IdentityStatus::Active,
                    created_at: now,
                    last_seen_at: now,
                    deleted_at: None,
                    reusable_after: None,
                    properties: None,
                };
                (account, identity, condition, true)
            }
        };
        let mut identity = identity;
        identity.properties = Some(input.properties);
        Ok(PreparedProviderIssuance {
            account,
            identity,
            identity_condition,
            new_account,
            session: session.clone(),
            code_ttl,
        })
    }

    pub async fn finish_provider_issuance(
        &self,
        prepared: PreparedProviderIssuance,
        code_digest: &str,
    ) -> Result<Subject, StorageError> {
        let PreparedProviderIssuance {
            account,
            mut identity,
            identity_condition,
            new_account,
            session,
            code_ttl,
        } = prepared;
        let now = (self.provider_issuance_now)();
        if now >= session.expires_at || code_digest.is_empty() {
            return Err(stale_sign_in());
        }
        let expires_at = now
            .checked_add_signed(code_ttl)
            .ok_or_else(|| StorageError::DynamoDB("invalid authorization code TTL".into()))?;
        identity.last_seen_at = now;
        let properties = identity.properties.as_ref().expect("prepared properties");
        let code = AuthorizationCodeRecord {
            credential_generation: account.credential_generation,
            client_id: session.client_id,
            redirect_uri: session.redirect_uri,
            subject: account.subject.clone(),
            subject_type: "user".into(),
            properties: json!({
                "provider": identity.provider,
                "email": properties.get("email").and_then(Value::as_str),
                "email_verified": properties.get("email_verified").and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
            code_challenge: session.code_challenge,
            code_challenge_method: session.code_challenge_method,
            scope: session.scope,
            oidc_nonce: session.oidc_nonce,
            created_at: now,
            expires_at,
        };
        let mut operations = if new_account {
            let index = IdentitySubjectIndexRecord {
                provider: identity.provider.clone(),
                identity_digest: identity.identity_digest.clone(),
                subject: account.subject.clone(),
                created_at: identity.created_at,
            };
            vec![
                TransactOperation::Put {
                    key: StoreKey::account(&account.subject).parts(),
                    value: to_value(&account)?,
                    expiry: None,
                    condition: Some(TransactCondition::NotExists),
                },
                TransactOperation::Put {
                    key: StoreKey::identity_by_subject(
                        &account.subject,
                        &identity.provider,
                        &identity.identity_digest,
                    )
                    .parts(),
                    value: to_value(&index)?,
                    expiry: None,
                    condition: Some(TransactCondition::NotExists),
                },
            ]
        } else {
            vec![guard(&account)?]
        };
        operations.extend([
            TransactOperation::Put {
                key: StoreKey::identity(&identity.provider, &identity.identity_digest).parts(),
                value: to_value(&identity)?,
                expiry: None,
                condition: Some(identity_condition),
            },
            TransactOperation::Put {
                key: StoreKey::authorization_code(code_digest).parts(),
                value: to_value(&code)?,
                expiry: Some(expires_at),
                condition: Some(TransactCondition::NotExists),
            },
        ]);
        if (self.provider_issuance_now)() >= session.expires_at {
            return Err(stale_sign_in());
        }
        self.storage.transact(operations).await?;
        Ok(Subject::from_persisted(account.subject))
    }
}

fn stale_sign_in() -> StorageError {
    StorageError::ConditionFailed("provider sign-in changed or expired".into())
}
