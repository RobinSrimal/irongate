//! Password policy and email normalization primitives.

use crate::crypto::password::hash_password;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub max_length: usize,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 12,
            max_length: 128,
        }
    }
}

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password must be at least {min_length} characters")]
    TooShort { min_length: usize },

    #[error("password must be at most {max_length} characters")]
    TooLong { max_length: usize },

    #[error("email address is invalid")]
    InvalidEmail,

    #[error("password hash failed: {0}")]
    Hash(String),
}

pub fn validate_password(password: &str, policy: &PasswordPolicy) -> Result<(), PasswordError> {
    let length = password.chars().count();
    if length < policy.min_length {
        return Err(PasswordError::TooShort {
            min_length: policy.min_length,
        });
    }
    if length > policy.max_length {
        return Err(PasswordError::TooLong {
            max_length: policy.max_length,
        });
    }

    Ok(())
}

pub fn hash_password_for_storage(password: &str) -> Result<String, PasswordError> {
    validate_password(password, &PasswordPolicy::default())?;
    hash_password(password).map_err(|err| PasswordError::Hash(err.to_string()))
}

pub fn normalize_email(email: &str) -> Result<String, PasswordError> {
    let trimmed = email.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return Err(PasswordError::InvalidEmail);
    }

    let mut parts = trimmed.split('@');
    let local = parts.next().ok_or(PasswordError::InvalidEmail)?;
    let domain = parts.next().ok_or(PasswordError::InvalidEmail)?;
    if parts.next().is_some()
        || local.is_empty()
        || domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
    {
        return Err(PasswordError::InvalidEmail);
    }

    Ok(format!(
        "{}@{}",
        local.to_ascii_lowercase(),
        domain.to_ascii_lowercase()
    ))
}
