//! Internal Google mailbox-proof delivery. Not wired to a public callback or endpoint.

use super::verified_email::GoogleEmailCandidate;
use crate::{
    config::{environment::RuntimeAuthConfig, Endpoint, RateLimitConfig},
    crypto::random::{generate_random_string, generate_unbiased_digits},
    email::{
        render_sign_in_confirmation_email, SignInConfirmationEmailInput, VerificationEmailSender,
    },
    error::{AuthError, StorageError},
    store::{
        rate_limits::{RateLimitIdentity, RateLimitSubject},
        records::AuthorizeSessionRecord,
        sign_in_confirmation::{valid_handle, valid_session, ConfirmationDigests},
        AuthStore,
    },
};
use chrono::{DateTime, Utc};
use thiserror::Error;

/// Caller has validated Google claims and consumed browser-bound provider/session state.
pub struct GoogleConfirmationInput<'a> {
    pub candidate: GoogleEmailCandidate,
    pub session: &'a AuthorizeSessionRecord,
    pub browser_proof: &'a str,
    pub trusted_source: Option<&'a str>,
}

/// Deliberately neither Debug nor Serialize: the handle is a short-lived secret.
pub struct PendingGoogleConfirmation {
    pub request: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ConfirmationDeliveryError {
    #[error("sign-in confirmation is invalid or no longer eligible")]
    InvalidConfirmation,
    #[error("sign-in confirmation expired")]
    Expired,
    #[error("sign-in confirmation storage unavailable")]
    StorageUnavailable,
    #[error("sign-in confirmation delivery unavailable")]
    DeliveryUnavailable,
    #[error(transparent)]
    Admission(#[from] AuthError),
}

impl From<StorageError> for ConfirmationDeliveryError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::ConditionFailed(_) | StorageError::AlreadyExists(_) => {
                Self::InvalidConfirmation
            }
            _ => Self::StorageUnavailable,
        }
    }
}

pub async fn begin_google_confirmation<E: VerificationEmailSender + ?Sized>(
    store: &AuthStore,
    runtime: &RuntimeAuthConfig,
    limits: &RateLimitConfig,
    sender: &E,
    input: GoogleConfirmationInput<'_>,
) -> Result<PendingGoogleConfirmation, ConfirmationDeliveryError> {
    if !valid_handle(input.browser_proof) || !valid_session(input.session) {
        return Err(ConfirmationDeliveryError::InvalidConfirmation);
    }
    let now = store.confirmation_now();
    if now < input.session.created_at || now >= input.session.expires_at {
        return Err(ConfirmationDeliveryError::Expired);
    }
    let email = input.candidate.email().to_owned();
    let secret = runtime.lookup_secret.as_bytes();
    store
        .check_rate_limit(
            limits,
            Endpoint::ProviderEmailSend,
            RateLimitIdentity {
                secret,
                source: input.trusted_source,
                subject: RateLimitSubject::Email(&email),
            },
        )
        .await?;

    let request = generate_random_string(32);
    let code = generate_unbiased_digits(8);
    let digests = ConfirmationDigests::from_secrets(secret, &request, input.browser_proof, &code)?;
    let expires_at = store
        .create_sign_in_confirmation(
            input.candidate,
            input.session,
            &digests,
            runtime.ttls.provider_state_seconds,
            runtime.account_lifecycle.deleted_identity_reuse,
        )
        .await?;
    let message = render_sign_in_confirmation_email(SignInConfirmationEmailInput {
        config: &runtime.email,
        code: &code,
        expires_at,
    });
    if store.confirmation_now() >= expires_at {
        return Err(ConfirmationDeliveryError::Expired);
    }
    // Delivery is outside the transaction. An error may follow acceptance; never undo or resend.
    sender
        .send_verification_email(&email, message)
        .await
        .map_err(|_| ConfirmationDeliveryError::DeliveryUnavailable)?;
    if store.confirmation_now() >= expires_at {
        return Err(ConfirmationDeliveryError::Expired);
    }
    Ok(PendingGoogleConfirmation {
        request,
        expires_at,
    })
}
