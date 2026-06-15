//! Email message rendering for API-only auth flows.

use crate::config::email::EmailConfig;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEmail {
    pub subject: String,
    pub html: String,
    pub text: String,
}

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

#[derive(Debug, Clone, Copy)]
pub struct VerificationEmailInput<'a> {
    pub config: &'a EmailConfig,
    pub email: &'a str,
    pub verification_token: &'a str,
    pub expires_minutes: u64,
}

pub fn render_verification_email(input: VerificationEmailInput<'_>) -> RenderedEmail {
    let verification_url = verification_url(input.config, input.verification_token);
    let brand_html = html_escape(&input.config.brand_name);
    let email_html = html_escape(input.email);
    let url_html = html_escape(&verification_url);
    let support_html = input
        .config
        .support_email
        .as_deref()
        .map(html_escape)
        .unwrap_or_else(|| "support".to_string());

    let html = format!(
        r#"<!doctype html>
<html>
  <body>
    <p>Use this link to verify your {brand} account for {email}:</p>
    <p><a href="{url}">Verify email address</a></p>
    <p>This link expires in {expires} minutes.</p>
    <p>If you did not request this email, contact {support}.</p>
  </body>
</html>"#,
        brand = brand_html,
        email = email_html,
        url = url_html,
        expires = input.expires_minutes,
        support = support_html
    );

    let support_text = input.config.support_email.as_deref().unwrap_or("support");
    let text = format!(
        "Use this link to verify your {brand} account for {email}:\n\n{url}\n\nThis link expires in {expires} minutes.\n\nIf you did not request this email, contact {support}.",
        brand = input.config.brand_name,
        email = input.email,
        url = verification_url,
        expires = input.expires_minutes,
        support = support_text
    );

    RenderedEmail {
        subject: input.config.verify_subject.clone(),
        html,
        text,
    }
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

fn verification_url(config: &EmailConfig, token: &str) -> String {
    let mut url = config.verify_url_base.clone();
    url.query_pairs_mut().append_pair("token", token);
    url.to_string()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
