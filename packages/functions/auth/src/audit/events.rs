//! Allowlisted outcomes, not arbitrary names or free-form request/error data.
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AuditEvent<'a> {
    AuthorizationCodeExchanged {
        client_id: &'a str,
        subject: &'a str,
    },
    RefreshTokenIssued {
        client_id: &'a str,
        subject: &'a str,
        token_hash: &'a str,
    },
    RefreshTokenRotated {
        client_id: &'a str,
        subject: &'a str,
        token_hash: &'a str,
    },
    RefreshTokenReuse {
        client_id: &'a str,
        token_hash: &'a str,
    },
    RefreshTokenRevoked {
        client_id: &'a str,
        token_hash: &'a str,
        reason: RevocationReason,
    },
    AdminAccountRead {
        actor: &'a str,
        subject: &'a str,
    },
    AdminAccountDisabled {
        actor: &'a str,
        subject: &'a str,
        revoked_refresh_families: usize,
        deleted_password_secrets: usize,
    },
    AdminAccountEnabled {
        actor: &'a str,
        subject: &'a str,
        revoked_refresh_families: usize,
        deleted_password_secrets: usize,
    },
    AdminAccountDeleted {
        actor: &'a str,
        subject: &'a str,
        revoked_refresh_families: usize,
        deleted_identities: usize,
        deleted_password_users: usize,
        deleted_password_secrets: usize,
    },
    AdminSubjectSessionsRevoked {
        actor: &'a str,
        subject: &'a str,
        revoked_refresh_families: usize,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevocationReason {
    Revoked,
    AlreadyRevoked,
}

impl AuditEvent<'_> {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::AuthorizationCodeExchanged { .. } => "authorization_code_exchanged",
            Self::RefreshTokenIssued { .. } => "refresh_token_issued",
            Self::RefreshTokenRotated { .. } => "refresh_token_rotated",
            Self::RefreshTokenReuse { .. } => "refresh_token_reuse",
            Self::RefreshTokenRevoked { .. } => "refresh_token_revoked",
            Self::AdminAccountRead { .. } => "admin_account_read",
            Self::AdminAccountDisabled { .. } => "admin_account_disabled",
            Self::AdminAccountEnabled { .. } => "admin_account_enabled",
            Self::AdminAccountDeleted { .. } => "admin_account_deleted",
            Self::AdminSubjectSessionsRevoked { .. } => "admin_subject_sessions_revoked",
        }
    }
    pub(super) fn is_valid(&self) -> bool {
        match self {
            Self::AuthorizationCodeExchanged { client_id, subject } => {
                client(client_id) && internal_subject(subject)
            }
            Self::RefreshTokenIssued {
                client_id,
                subject,
                token_hash,
            }
            | Self::RefreshTokenRotated {
                client_id,
                subject,
                token_hash,
            } => client(client_id) && internal_subject(subject) && digest(token_hash),
            Self::RefreshTokenReuse {
                client_id,
                token_hash,
            }
            | Self::RefreshTokenRevoked {
                client_id,
                token_hash,
                ..
            } => client(client_id) && digest(token_hash),
            Self::AdminAccountRead { actor, subject }
            | Self::AdminAccountDisabled { actor, subject, .. }
            | Self::AdminAccountEnabled { actor, subject, .. }
            | Self::AdminAccountDeleted { actor, subject, .. }
            | Self::AdminSubjectSessionsRevoked { actor, subject, .. } => {
                internal_subject(subject) && identifier(actor, 2048, b"-_.:/=+@")
            }
        }
    }
}
fn client(value: &str) -> bool {
    identifier(value, 256, b"-_.:")
}
fn internal_subject(value: &str) -> bool {
    identifier(value, 128, b"-_")
}
fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn identifier(value: &str, max: usize, punctuation: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || punctuation.contains(&byte))
}
