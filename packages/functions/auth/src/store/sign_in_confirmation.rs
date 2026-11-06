//! Durable email proof consumed atomically with a pinned Google identity plan. No HTTP surface.

use super::{
    credential_generation::equal,
    keys::StoreKey,
    provider_issuance::{code_lifetime, code_operation},
    provider_plan::NewProviderPlan,
    records::AuthorizeSessionRecord,
    to_value, AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
};
use crate::{
    core::{passwords::normalize_email, subjects::Subject},
    crypto::hmac_lookup::{lookup_digest, LookupFamily},
    error::StorageError,
    providers::verified_email::GoogleEmailCandidate,
    storage::TransactOperation,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use subtle::ConstantTimeEq;

const PURPOSE: &str = "google_sign_in_confirmation";

/// Digests only, not evidence that the mailbox has already been verified.
pub struct ConfirmationDigests {
    request: String,
    browser: String,
    email_proof: String,
}

impl ConfirmationDigests {
    pub fn from_secrets(
        secret: &[u8],
        request: &str,
        browser: &str,
        email_proof: &str,
    ) -> Result<Self, StorageError> {
        if !valid_handle(request)
            || !valid_handle(browser)
            || email_proof.is_empty()
            || email_proof.len() > 128
            || !email_proof
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(rejected());
        }
        let request = lookup_digest(secret, LookupFamily::SignInConfirmation, request);
        Ok(Self {
            email_proof: email_proof_digest(secret, &request, email_proof),
            browser: lookup_digest(secret, LookupFamily::SignInBrowser, browser),
            request,
        })
    }

    pub fn request_digest(&self) -> &str {
        &self.request
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmationRecord {
    purpose: String,
    request_digest: String,
    identity_digest: String,
    email: String,
    email_digest: String,
    browser_digest: String,
    email_proof_digest: String,
    session: AuthorizeSessionRecord,
    resolution: NewProviderPlan,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

pub struct PreparedSignInConfirmation {
    record: ConfirmationRecord,
    original: Value,
}

/// Browser admission permits email-budget selection, not identity/code issuance.
pub struct SignInConfirmationAttempt {
    record: ConfirmationRecord,
    original: Value,
}

impl SignInConfirmationAttempt {
    pub(crate) fn email(&self) -> &str {
        &self.record.email
    }
}

impl PreparedSignInConfirmation {
    pub(crate) fn continuation(&self) -> (&str, Option<&str>) {
        (
            &self.record.session.redirect_uri,
            self.record.session.state.as_deref(),
        )
    }
}

impl AuthStore {
    pub(crate) fn confirmation_now(&self) -> DateTime<Utc> {
        (self.provider_issuance_now)()
    }

    pub async fn create_sign_in_confirmation(
        &self,
        candidate: GoogleEmailCandidate,
        session: &AuthorizeSessionRecord,
        digests: &ConfirmationDigests,
        lifetime_seconds: u64,
        policy: DeletedIdentityReusePolicy,
    ) -> Result<DateTime<Utc>, StorageError> {
        let now = (self.provider_issuance_now)();
        if !valid_session(session) || now < session.created_at || now >= session.expires_at {
            return Err(rejected());
        }
        let ttl = i64::try_from(lifetime_seconds)
            .ok()
            .filter(|ttl| *ttl > 0)
            .and_then(Duration::try_seconds)
            .ok_or_else(rejected)?;
        let expires_at = now
            .checked_add_signed(ttl)
            .ok_or_else(rejected)?
            .min(session.expires_at);
        let (identity_digest, email_digest, email) = candidate.into_parts();
        let existing = self
            .read_provider_identity(IdentityProvider::Google, &identity_digest)
            .await?;
        let resolution = self
            .prepare_new_provider_plan(
                IdentityProvider::Google,
                &identity_digest,
                existing,
                &email_digest,
                policy,
            )
            .await?;
        let created_at = resolution.created_at();
        resolution.validate_google_snapshot(&identity_digest, &email_digest, created_at)?;
        let mut operations = resolution.creation_guards()?;
        let record = ConfirmationRecord {
            purpose: PURPOSE.into(),
            request_digest: digests.request.clone(),
            identity_digest,
            email,
            email_digest,
            browser_digest: digests.browser.clone(),
            email_proof_digest: digests.email_proof.clone(),
            session: session.clone(),
            resolution,
            created_at,
            expires_at,
        };
        operations.push(TransactOperation::Put {
            key: StoreKey::sign_in_confirmation(&digests.request).parts(),
            value: to_value(&record)?,
            expiry: Some(expires_at),
            condition: Some(crate::storage::TransactCondition::NotExists),
        });
        if (self.provider_issuance_now)() >= expires_at {
            return Err(rejected());
        }
        self.storage.transact(operations).await?;
        Ok(expires_at)
    }

    pub async fn load_sign_in_confirmation_attempt(
        &self,
        request: &str,
        browser: &str,
        lookup_secret: &[u8],
    ) -> Result<Option<SignInConfirmationAttempt>, StorageError> {
        if !valid_handle(request) || !valid_handle(browser) {
            return Ok(None);
        }
        let request = lookup_digest(lookup_secret, LookupFamily::SignInConfirmation, request);
        let browser = lookup_digest(lookup_secret, LookupFamily::SignInBrowser, browser);
        let Some(original): Option<Value> = self
            .get_record(&StoreKey::sign_in_confirmation(&request))
            .await?
        else {
            return Ok(None);
        };
        let record: ConfirmationRecord =
            serde_json::from_value(original.clone()).map_err(|_| invalid_record())?;
        record.validate(&request, lookup_secret)?;
        if !record.is_live(self.confirmation_now()) {
            return Ok(None);
        }
        if !bool::from(record.browser_digest.as_bytes().ct_eq(browser.as_bytes())) {
            return Ok(None);
        }
        Ok(Some(SignInConfirmationAttempt { record, original }))
    }

    /// Call only after attempt admission. The returned type alone permits finishing.
    pub fn verify_sign_in_confirmation_attempt(
        &self,
        attempt: SignInConfirmationAttempt,
        code: &str,
        lookup_secret: &[u8],
    ) -> Option<PreparedSignInConfirmation> {
        let SignInConfirmationAttempt { record, original } = attempt;
        if !record.is_live(self.confirmation_now())
            || code.len() != 8
            || !code.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let digest = email_proof_digest(lookup_secret, &record.request_digest, code);
        let matches = record
            .email_proof_digest
            .as_bytes()
            .ct_eq(digest.as_bytes());
        bool::from(matches).then_some(PreparedSignInConfirmation { record, original })
    }

    pub async fn finish_sign_in_confirmation(
        &self,
        prepared: PreparedSignInConfirmation,
        code_digest: &str,
        code_ttl_seconds: u64,
    ) -> Result<Subject, StorageError> {
        let PreparedSignInConfirmation { record, original } = prepared;
        let now = (self.provider_issuance_now)();
        if now >= record.expires_at
            || now >= record.session.expires_at
            || !valid_digest(code_digest)
        {
            return Err(rejected());
        }
        let properties = json!({"provider":"google", "email":record.email, "email_verified":true});
        let mut operations = record.resolution.operations(properties.clone(), now)?;
        operations.push(code_operation(
            &record.resolution.account,
            "google",
            &properties,
            &record.session,
            code_digest,
            code_lifetime(code_ttl_seconds)?,
            now,
        )?);
        operations.push(TransactOperation::Delete {
            key: StoreKey::sign_in_confirmation(&record.request_digest).parts(),
            condition: Some(equal(&original)?),
        });
        if (self.provider_issuance_now)() >= record.expires_at {
            return Err(rejected());
        }
        self.storage.transact(operations).await?;
        Ok(Subject::from_persisted(record.resolution.account.subject))
    }
}

impl ConfirmationRecord {
    fn is_live(&self, now: DateTime<Utc>) -> bool {
        now >= self.created_at && now < self.expires_at && now < self.session.expires_at
    }

    fn validate(&self, request_digest: &str, secret: &[u8]) -> Result<(), StorageError> {
        if self.purpose != PURPOSE
            || self.request_digest != request_digest
            || ![
                &self.request_digest,
                &self.identity_digest,
                &self.email_digest,
                &self.browser_digest,
                &self.email_proof_digest,
            ]
            .into_iter()
            .all(|s| valid_digest(s))
            || normalize_email(&self.email).ok().as_deref() != Some(self.email.as_str())
            || lookup_digest(secret, LookupFamily::Email, &self.email) != self.email_digest
            || !valid_session(&self.session)
            || self.created_at < self.session.created_at
            || self.expires_at <= self.created_at
            || self.expires_at > self.session.expires_at
        {
            return Err(invalid_record());
        }
        self.resolution.validate_google_snapshot(
            &self.identity_digest,
            &self.email_digest,
            self.created_at,
        )
    }
}

fn email_proof_digest(secret: &[u8], request_digest: &str, code: &str) -> String {
    // The request digest is fixed-length hex; the delimiter cannot occur inside it.
    lookup_digest(
        secret,
        LookupFamily::SignInConfirmationCode,
        &format!("{request_digest}:{code}"),
    )
}

pub(crate) fn valid_session(session: &AuthorizeSessionRecord) -> bool {
    session.selected_provider.as_deref() == Some("google")
        && !session.client_id.trim().is_empty()
        && !session.redirect_uri.is_empty()
        && !session.scope.trim().is_empty()
        && session.created_at < session.expires_at
}
pub(crate) fn valid_handle(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn rejected() -> StorageError {
    StorageError::ConditionFailed("sign-in confirmation changed or expired".into())
}
fn invalid_record() -> StorageError {
    StorageError::DynamoDB("invalid sign-in confirmation record".into())
}
