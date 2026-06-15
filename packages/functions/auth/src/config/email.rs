//! Email delivery configuration for registration and verification flows.

use std::fmt;
use std::path::PathBuf;
use thiserror::Error;
use url::Url;

#[derive(Clone, PartialEq, Eq)]
pub struct EmailSecret(String);

impl EmailSecret {
    pub fn new(value: String, name: &'static str) -> Result<Self, EmailConfigError> {
        if value.trim().is_empty() {
            return Err(EmailConfigError::Missing { name });
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for EmailSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmailSecret")
            .field("present", &true)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmailConfig {
    pub resend_api_key: EmailSecret,
    pub from: String,
    pub verify_url_base: Url,
    pub reset_url_base: Url,
    pub reply_to: Option<String>,
    pub brand_name: String,
    pub support_email: Option<String>,
    pub verify_subject: String,
    pub reset_subject: String,
    pub verify_template_path: Option<PathBuf>,
    pub reset_template_path: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum EmailConfigError {
    #[error("{name} is required")]
    Missing { name: &'static str },

    #[error("{name} must be a valid URL: {source}")]
    InvalidUrl {
        name: &'static str,
        source: url::ParseError,
    },
}

impl EmailConfig {
    pub fn from_env_map(
        vars: &std::collections::HashMap<String, String>,
    ) -> Result<Self, EmailConfigError> {
        let resend_api_key = required_secret(vars, "RESEND_API_KEY")?;
        let from = required_string(vars, "AUTH_EMAIL_FROM")?;
        let verify_url_base = required_url(vars, "AUTH_EMAIL_VERIFY_URL_BASE")?;
        let reset_url_base = required_url(vars, "AUTH_EMAIL_RESET_URL_BASE")?;

        Ok(Self {
            resend_api_key,
            from,
            verify_url_base,
            reset_url_base,
            reply_to: optional_string(vars, "AUTH_EMAIL_REPLY_TO"),
            brand_name: optional_string(vars, "AUTH_EMAIL_BRAND_NAME")
                .unwrap_or_else(|| "Irongate".to_string()),
            support_email: optional_string(vars, "AUTH_EMAIL_SUPPORT_EMAIL"),
            verify_subject: optional_string(vars, "AUTH_EMAIL_VERIFY_SUBJECT")
                .unwrap_or_else(|| "Verify your email address".to_string()),
            reset_subject: optional_string(vars, "AUTH_EMAIL_RESET_SUBJECT")
                .unwrap_or_else(|| "Reset your password".to_string()),
            verify_template_path: optional_string(vars, "AUTH_EMAIL_VERIFY_TEMPLATE_PATH")
                .map(PathBuf::from),
            reset_template_path: optional_string(vars, "AUTH_EMAIL_RESET_TEMPLATE_PATH")
                .map(PathBuf::from),
        })
    }

    #[doc(hidden)]
    pub fn for_tests() -> Self {
        Self {
            resend_api_key: EmailSecret::new("re_test_key".to_string(), "RESEND_API_KEY")
                .expect("test resend key"),
            from: "Irongate <auth@example.com>".to_string(),
            verify_url_base: Url::parse("https://app.example.com/auth/verify-email")
                .expect("test verify url"),
            reset_url_base: Url::parse("https://app.example.com/auth/reset-password")
                .expect("test reset url"),
            reply_to: None,
            brand_name: "Irongate".to_string(),
            support_email: Some("support@example.com".to_string()),
            verify_subject: "Verify your email address".to_string(),
            reset_subject: "Reset your password".to_string(),
            verify_template_path: None,
            reset_template_path: None,
        }
    }
}

fn required_secret(
    vars: &std::collections::HashMap<String, String>,
    name: &'static str,
) -> Result<EmailSecret, EmailConfigError> {
    EmailSecret::new(required_string(vars, name)?, name)
}

fn required_string(
    vars: &std::collections::HashMap<String, String>,
    name: &'static str,
) -> Result<String, EmailConfigError> {
    optional_string(vars, name).ok_or(EmailConfigError::Missing { name })
}

fn optional_string(
    vars: &std::collections::HashMap<String, String>,
    name: &'static str,
) -> Option<String> {
    vars.get(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_url(
    vars: &std::collections::HashMap<String, String>,
    name: &'static str,
) -> Result<Url, EmailConfigError> {
    let value = required_string(vars, name)?;
    Url::parse(&value).map_err(|source| EmailConfigError::InvalidUrl { name, source })
}
