//! Resend email delivery implementation.

use crate::config::email::EmailConfig;
use crate::email::{EmailDeliveryError, RenderedEmail, VerificationEmailSender};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResendEmailRequest {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub html: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

#[derive(Clone)]
pub struct ResendEmailSender {
    config: EmailConfig,
    client: reqwest::Client,
    endpoint: String,
}

impl ResendEmailSender {
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            endpoint: "https://api.resend.com/emails".to_string(),
        }
    }

    #[doc(hidden)]
    pub fn with_endpoint(config: EmailConfig, endpoint: String) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            endpoint,
        }
    }
}

#[async_trait]
impl VerificationEmailSender for ResendEmailSender {
    async fn send_verification_email(
        &self,
        to: &str,
        message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        let request = build_resend_email_request(&self.config, to, &message);
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(self.config.resend_api_key.expose())
            .json(&request)
            .send()
            .await
            .map_err(EmailDeliveryError::Transport)?;

        let status = response.status();
        if !status.is_success() {
            return Err(EmailDeliveryError::ProviderStatus {
                status: status.as_u16(),
            });
        }

        let body = response
            .json::<ResendEmailResponse>()
            .await
            .map_err(EmailDeliveryError::InvalidResponse)?;
        if body.id.trim().is_empty() {
            return Err(EmailDeliveryError::MissingDeliveryId);
        }
        Ok(body.id)
    }
}

#[derive(Debug, Deserialize)]
struct ResendEmailResponse {
    id: String,
}

pub fn build_resend_email_request(
    config: &EmailConfig,
    to: &str,
    message: &RenderedEmail,
) -> ResendEmailRequest {
    ResendEmailRequest {
        from: config.from.clone(),
        to: vec![to.to_string()],
        subject: message.subject.clone(),
        html: message.html.clone(),
        text: message.text.clone(),
        reply_to: config.reply_to.clone(),
    }
}
