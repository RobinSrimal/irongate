//! Verification, reset and sign-in confirmation email rendering.

use crate::config::email::EmailConfig;
use crate::email::RenderedEmail;
use chrono::{DateTime, SecondsFormat, Utc};

pub struct SignInConfirmationEmailInput<'a> {
    pub config: &'a EmailConfig,
    pub code: &'a str,
    pub expires_at: DateTime<Utc>,
}

pub fn render_sign_in_confirmation_email(input: SignInConfirmationEmailInput<'_>) -> RenderedEmail {
    let brand = &input.config.brand_name;
    let support = input.config.support_email.as_deref().unwrap_or("support");
    let expiry = input
        .expires_at
        .to_rfc3339_opts(SecondsFormat::AutoSi, true);
    RenderedEmail {
        subject: format!("{brand} sign-in code"),
        html: format!(
            "<p>Your {brand} sign-in code:</p><p><strong>{code}</strong></p><p>Expires at {expiry}.</p><p>Enter it only in the sign-in window you opened. Do not share it.</p><p>If you did not request this code, ignore this email. For help, contact {support}.</p>",
            brand = html_escape(brand), code = html_escape(input.code),
            expiry = html_escape(&expiry), support = html_escape(support),
        ),
        text: format!(
            "Your {brand} sign-in code:\n\n{code}\n\nExpires at {expiry}.\n\nEnter it only in the sign-in window you opened. Do not share it.\nIf you did not request this code, ignore this email. For help, contact {support}.",
            code = input.code,
        ),
    }
}

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
    <p>Choose a password to verify your email and finish setting up your {brand} account for {email}:</p>
    <p><a href="{url}">Complete account setup</a></p>
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
        "Choose a password to verify your email and finish setting up your {brand} account for {email}:\n\n{url}\n\nThis link expires in {expires} minutes.\n\nIf you did not request this email, contact {support}.",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_preserves_code_and_deadline_and_escapes_branding_without_links() {
        let mut config = EmailConfig::for_tests();
        config.brand_name = "<Brand & Name>".into();
        config.support_email = Some("<support@example.test>".into());
        let expires_at = "2030-01-02T03:04:05.123Z".parse().unwrap();
        let message = render_sign_in_confirmation_email(SignInConfirmationEmailInput {
            config: &config,
            code: "00123456",
            expires_at,
        });
        assert!(message.html.contains("&lt;Brand &amp; Name&gt;"));
        assert!(message.html.contains("&lt;support@example.test&gt;"));
        assert!(!message.html.contains("<Brand"));
        assert!(message.text.contains(&config.brand_name));
        for body in [&message.html, &message.text] {
            assert_eq!(body.matches("00123456").count(), 1);
            assert!(body.contains("2030-01-02T03:04:05.123Z"));
            assert!(!body.contains("https://"));
            assert!(!body.contains("href="));
        }
    }
}
