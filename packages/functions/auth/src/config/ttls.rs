//! TTL configuration for tokens and short-lived auth artifacts.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtlConfig {
    pub access_token_seconds: u64,
    pub id_token_seconds: u64,
    pub refresh_token_seconds: u64,
    pub auth_code_seconds: u64,
    pub authorize_session_seconds: u64,
    pub provider_state_seconds: u64,
    pub email_verification_seconds: u64,
    pub password_reset_seconds: u64,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            access_token_seconds: 3_600,
            id_token_seconds: 3_600,
            refresh_token_seconds: 2_592_000,
            auth_code_seconds: 300,
            authorize_session_seconds: 600,
            provider_state_seconds: 600,
            email_verification_seconds: 900,
            password_reset_seconds: 900,
        }
    }
}

#[derive(Debug, Error)]
pub enum TtlConfigError {
    #[error("TTL `{name}` must be positive")]
    NonPositive { name: &'static str },

    #[error("access-token TTL must be shorter than refresh-token TTL")]
    AccessLongerThanRefresh,

    #[error("ID-token TTL must not exceed refresh-token TTL")]
    IdLongerThanRefresh,

    #[error("authorization-code TTL must not exceed authorize-session TTL")]
    CodeLongerThanSession,

    #[error("provider-state TTL must not exceed authorize-session TTL")]
    StateLongerThanSession,
}

impl TtlConfig {
    pub fn validate(&self) -> Result<(), TtlConfigError> {
        for (name, value) in [
            ("access_token", self.access_token_seconds),
            ("id_token", self.id_token_seconds),
            ("refresh_token", self.refresh_token_seconds),
            ("auth_code", self.auth_code_seconds),
            ("authorize_session", self.authorize_session_seconds),
            ("provider_state", self.provider_state_seconds),
            ("email_verification", self.email_verification_seconds),
            ("password_reset", self.password_reset_seconds),
        ] {
            if value == 0 {
                return Err(TtlConfigError::NonPositive { name });
            }
        }

        if self.access_token_seconds >= self.refresh_token_seconds {
            return Err(TtlConfigError::AccessLongerThanRefresh);
        }
        if self.id_token_seconds > self.refresh_token_seconds {
            return Err(TtlConfigError::IdLongerThanRefresh);
        }
        if self.auth_code_seconds > self.authorize_session_seconds {
            return Err(TtlConfigError::CodeLongerThanSession);
        }
        if self.provider_state_seconds > self.authorize_session_seconds {
            return Err(TtlConfigError::StateLongerThanSession);
        }

        Ok(())
    }
}
