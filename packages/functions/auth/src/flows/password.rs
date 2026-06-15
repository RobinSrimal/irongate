//! Password registration and email verification domain flow.

use crate::config::environment::RuntimeAuthConfig;
use crate::core::passwords::{
    hash_password_for_storage, normalize_email, validate_password, PasswordError, PasswordPolicy,
};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::crypto::random::generate_random_string;
use crate::email::{
    render_verification_email, EmailDeliveryError, VerificationEmailInput, VerificationEmailSender,
};
use crate::error::StorageError;
use crate::storage::StorageAdapter;
use crate::store::AuthStore;
use crate::store::IdentityProvider;
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct PasswordRegistrationInput<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PasswordRegistrationStatus {
    VerificationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasswordRegistrationOutcome {
    pub status: PasswordRegistrationStatus,
    pub delivery_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct PasswordVerificationInput<'a> {
    pub token: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PasswordVerificationStatus {
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasswordVerificationOutcome {
    pub status: PasswordVerificationStatus,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

#[derive(Debug, Error)]
pub enum PasswordRegistrationError {
    #[error(transparent)]
    Password(#[from] PasswordError),

    #[error("email is already registered")]
    EmailAlreadyRegistered,

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("email delivery failed")]
    EmailDelivery(#[from] EmailDeliveryError),
}

#[derive(Debug, Error)]
pub enum PasswordVerificationError {
    #[error("verification token is invalid or expired")]
    InvalidVerificationToken,

    #[error("password user was not found")]
    PasswordUserNotFound,

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

pub async fn register_password_user<S, E>(
    store: &AuthStore<S>,
    runtime: &RuntimeAuthConfig,
    sender: &E,
    input: PasswordRegistrationInput<'_>,
) -> Result<PasswordRegistrationOutcome, PasswordRegistrationError>
where
    S: StorageAdapter,
    E: VerificationEmailSender + ?Sized,
{
    let email = normalize_email(input.email)?;
    validate_password(input.password, &PasswordPolicy::default())?;

    let email_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::Email,
        &email,
    );

    match store
        .get_password_user_by_email_digest(&email_digest)
        .await?
    {
        Some(existing) if existing.verified => {
            return Err(PasswordRegistrationError::EmailAlreadyRegistered);
        }
        Some(_) => {}
        None => {
            let password_hash = hash_password_for_storage(input.password)?;
            store
                .create_unverified_password_user(&email_digest, &email, &password_hash)
                .await?;
        }
    }

    let verification_token = generate_random_string(43);
    let verification_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        &verification_token,
    );
    let expires_at = Utc::now() + Duration::seconds(runtime.ttls.email_verification_seconds as i64);
    store
        .create_email_verification(&verification_digest, &email_digest, expires_at)
        .await?;

    let rendered = render_verification_email(VerificationEmailInput {
        config: &runtime.email,
        email: &email,
        verification_token: &verification_token,
        expires_minutes: runtime.ttls.email_verification_seconds.div_ceil(60),
    });
    let delivery_id = sender.send_verification_email(&email, rendered).await?;

    Ok(PasswordRegistrationOutcome {
        status: PasswordRegistrationStatus::VerificationRequired,
        delivery_id,
        authorization_code: None,
        access_token: None,
    })
}

pub async fn verify_password_email<S>(
    store: &AuthStore<S>,
    runtime: &RuntimeAuthConfig,
    input: PasswordVerificationInput<'_>,
) -> Result<PasswordVerificationOutcome, PasswordVerificationError>
where
    S: StorageAdapter,
{
    let verification_digest = lookup_digest(
        runtime.lookup_secret.as_bytes(),
        LookupFamily::EmailVerification,
        input.token,
    );
    let verification = store
        .consume_email_verification(&verification_digest)
        .await?
        .ok_or(PasswordVerificationError::InvalidVerificationToken)?;

    let password_user = store
        .get_password_user_by_email_digest(&verification.email_digest)
        .await?
        .ok_or(PasswordVerificationError::PasswordUserNotFound)?;

    let subject = if password_user.verified {
        password_user
            .subject
            .ok_or(PasswordVerificationError::PasswordUserNotFound)?
    } else {
        let identity_digest = lookup_digest(
            runtime.lookup_secret.as_bytes(),
            LookupFamily::PasswordIdentity,
            &password_user.email,
        );
        let subject = store
            .verify_password_user_with_identity(
                &verification.email_digest,
                IdentityProvider::Password,
                &identity_digest,
                json!({
                    "email": password_user.email,
                    "email_verified": true,
                }),
            )
            .await?;
        subject.as_str().to_string()
    };

    Ok(PasswordVerificationOutcome {
        status: PasswordVerificationStatus::Verified,
        subject,
        authorization_code: None,
        access_token: None,
    })
}
