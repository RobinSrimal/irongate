//! Email delivery and template rendering for API-only auth flows.

use async_trait::async_trait;
use thiserror::Error;

pub mod resend;
pub mod templates;

#[allow(unused_imports)]
pub use resend::{build_resend_email_request, ResendEmailRequest, ResendEmailSender};
pub use templates::{render_verification_email, VerificationEmailInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEmail {
    pub subject: String,
    pub html: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum EmailDeliveryError {
    #[error("email delivery transport failed")]
    Transport(#[source] reqwest::Error),

    #[error("email delivery provider returned status {status}")]
    ProviderStatus { status: u16 },

    #[error("email delivery response was invalid")]
    InvalidResponse(#[source] reqwest::Error),

    #[error("email delivery response did not include a delivery id")]
    MissingDeliveryId,
}

#[async_trait]
pub trait VerificationEmailSender: Send + Sync {
    async fn send_verification_email(
        &self,
        to: &str,
        message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct NoopEmailSender;

#[cfg(test)]
#[async_trait]
impl VerificationEmailSender for NoopEmailSender {
    async fn send_verification_email(
        &self,
        _to: &str,
        _message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        Ok("noop-delivery".to_string())
    }
}
