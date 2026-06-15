//! Verification and reset email template rendering.

use crate::config::email::EmailConfig;
use crate::email::RenderedEmail;

#[derive(Debug, Clone, Copy)]
pub struct VerificationEmailInput<'a> {
    pub config: &'a EmailConfig,
    pub email: &'a str,
    pub verification_token: &'a str,
    pub expires_minutes: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct PasswordResetEmailInput<'a> {
    pub config: &'a EmailConfig,
    pub email: &'a str,
    pub reset_token: &'a str,
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

pub fn render_password_reset_email(input: PasswordResetEmailInput<'_>) -> RenderedEmail {
    let reset_url = password_reset_url(input.config, input.reset_token);
    let brand_html = html_escape(&input.config.brand_name);
    let email_html = html_escape(input.email);
    let url_html = html_escape(&reset_url);
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
    <p>Use this link to reset your {brand} password for {email}:</p>
    <p><a href="{url}">Reset password</a></p>
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
        "Use this link to reset your {brand} password for {email}:\n\n{url}\n\nThis link expires in {expires} minutes.\n\nIf you did not request this email, contact {support}.",
        brand = input.config.brand_name,
        email = input.email,
        url = reset_url,
        expires = input.expires_minutes,
        support = support_text
    );

    RenderedEmail {
        subject: input.config.reset_subject.clone(),
        html,
        text,
    }
}

fn verification_url(config: &EmailConfig, token: &str) -> String {
    let mut url = config.verify_url_base.clone();
    url.query_pairs_mut().append_pair("token", token);
    url.to_string()
}

fn password_reset_url(config: &EmailConfig, token: &str) -> String {
    let mut url = config.reset_url_base.clone();
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
