//! Provider identity updates and authorization-code issuance share one commit.

use super::{
    credential_generation::{equal, guard},
    keys::StoreKey,
    provider_plan::NewProviderPlan,
    records::{
        AccountRecord, AuthorizationCodeRecord, AuthorizeSessionRecord, IdentityRecord,
        IdentityStatus,
    },
    to_value, AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
};
use crate::{
    core::subjects::Subject,
    error::StorageError,
    providers::verified_email::ProviderVerifiedEmail,
    storage::{TransactCondition, TransactOperation},
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

/// Supplied by the callback only after full provider-token validation.
pub struct ProviderIdentityInput<'a> {
    pub provider: IdentityProvider,
    pub identity_digest: &'a str,
    pub properties: Value,
    pub verified_email: Option<ProviderVerifiedEmail>,
}

enum AccountPlan {
    Returning,
    NewIdentity(Box<NewProviderPlan>),
}

/// Private snapshots cannot be replaced by a caller between preparation and commit.
pub struct PreparedProviderIssuance {
    account: AccountRecord,
    identity: IdentityRecord,
    identity_condition: TransactCondition,
    account_plan: AccountPlan,
    session: AuthorizeSessionRecord,
    code_ttl: Duration,
}

impl AuthStore {
    pub async fn prepare_provider_issuance(
        &self,
        mut input: ProviderIdentityInput<'_>,
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
            || input
                .verified_email
                .as_ref()
                .is_some_and(|email| !email.matches_identity(input.provider, input.identity_digest))
        {
            return Err(stale_sign_in());
        }
        let code_ttl = code_lifetime(code_ttl_seconds)?;
        let existing = self
            .read_provider_identity(input.provider, input.identity_digest)
            .await?;

        let (account, identity, identity_condition, account_plan) = match existing {
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
                (account, identity, condition, AccountPlan::Returning)
            }
            existing => {
                let email = input.verified_email.as_ref().ok_or_else(stale_sign_in)?;
                let plan = self
                    .prepare_new_provider_plan(
                        input.provider,
                        input.identity_digest,
                        existing,
                        email.email_digest(),
                        reuse_policy,
                    )
                    .await?;
                let properties = input.properties.as_object_mut().ok_or_else(stale_sign_in)?;
                properties.insert("email".into(), json!(email.email()));
                properties.insert("email_verified".into(), json!(true));
                let identity = plan.identity(Value::Null, now);
                (
                    plan.account.clone(),
                    identity,
                    plan.condition()?,
                    AccountPlan::NewIdentity(Box::new(plan)),
                )
            }
        };
        let mut identity = identity;
        identity.properties = Some(input.properties);
        Ok(PreparedProviderIssuance {
            account,
            identity,
            identity_condition,
            account_plan,
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
            account_plan,
            session,
            code_ttl,
        } = prepared;
        let now = (self.provider_issuance_now)();
        if now >= session.expires_at || code_digest.is_empty() {
            return Err(stale_sign_in());
        }
        identity.last_seen_at = now;
        let properties = identity.properties.as_ref().expect("prepared properties");
        let mut operations = match account_plan {
            AccountPlan::Returning => vec![
                guard(&account)?,
                TransactOperation::Put {
                    key: StoreKey::identity(&identity.provider, &identity.identity_digest).parts(),
                    value: to_value(&identity)?,
                    expiry: None,
                    condition: Some(identity_condition),
                },
            ],
            AccountPlan::NewIdentity(plan) => plan.operations(properties.clone(), now)?,
        };
        operations.push(code_operation(
            &account,
            &identity.provider,
            properties,
            &session,
            code_digest,
            code_ttl,
            now,
        )?);
        if (self.provider_issuance_now)() >= session.expires_at {
            return Err(stale_sign_in());
        }
        self.storage.transact(operations).await?;
        Ok(Subject::from_persisted(account.subject))
    }
}

pub(super) fn code_lifetime(seconds: u64) -> Result<Duration, StorageError> {
    i64::try_from(seconds)
        .ok()
        .filter(|seconds| *seconds > 0)
        .and_then(Duration::try_seconds)
        .ok_or_else(|| StorageError::DynamoDB("invalid authorization code TTL".into()))
}

pub(super) fn code_operation(
    account: &AccountRecord,
    provider: &str,
    properties: &Value,
    session: &AuthorizeSessionRecord,
    code_digest: &str,
    ttl: Duration,
    now: DateTime<Utc>,
) -> Result<TransactOperation, StorageError> {
    let expires_at = now
        .checked_add_signed(ttl)
        .ok_or_else(|| StorageError::DynamoDB("invalid authorization code TTL".into()))?;
    let code = AuthorizationCodeRecord {
        credential_generation: account.credential_generation,
        client_id: session.client_id.clone(),
        redirect_uri: session.redirect_uri.clone(),
        subject: account.subject.clone(),
        subject_type: "user".into(),
        properties: json!({"provider":provider, "email":properties.get("email").and_then(Value::as_str),
            "email_verified":properties.get("email_verified").and_then(Value::as_bool).unwrap_or(false)}),
        code_challenge: session.code_challenge.clone(),
        code_challenge_method: session.code_challenge_method.clone(),
        scope: session.scope.clone(),
        oidc_nonce: session.oidc_nonce.clone(),
        created_at: now,
        expires_at,
    };
    Ok(TransactOperation::Put {
        key: StoreKey::authorization_code(code_digest).parts(),
        value: to_value(&code)?,
        expiry: Some(expires_at),
        condition: Some(TransactCondition::NotExists),
    })
}

fn stale_sign_in() -> StorageError {
    StorageError::ConditionFailed("provider sign-in changed or expired".into())
}
