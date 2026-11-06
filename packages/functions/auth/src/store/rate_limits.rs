//! Source-first, conservative multi-dimensional admission over bounded CAS counters.
use crate::config::{Endpoint, RateLimit, RateLimitConfig};
use crate::core::passwords::normalize_email;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::error::AuthError;
use crate::ratelimit::middleware::canonical_source_ip;
use crate::storage::StorageAdapter;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

const RATE_LIMIT_CAS_RETRIES: usize = 8;

/// None means an unconfigured client, never a caller-chosen sentinel string.
#[derive(Clone, Copy)]
pub enum RateLimitSubject<'a> {
    Email(&'a str),
    Client(Option<&'a str>),
    Provider {
        provider: &'a str,
        session_digest: &'a str,
    },
    None,
}

pub struct RateLimitIdentity<'a> {
    pub secret: &'a [u8],
    pub source: Option<&'a str>,
    pub subject: RateLimitSubject<'a>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitCounter {
    pub count: u32,
    pub window_start: DateTime<Utc>,
}

pub fn source_identifier(secret: &[u8], source: Option<&str>) -> String {
    let source = source.and_then(canonical_source_ip);
    format!(
        "source:{}",
        lookup_digest(
            secret,
            LookupFamily::RateLimitSource,
            source.as_deref().unwrap_or("unknown")
        )
    )
}

pub fn email_identifier(secret: &[u8], email: &str) -> Option<String> {
    normalize_email(email).ok().map(|email| {
        format!(
            "email:{}",
            lookup_digest(secret, LookupFamily::RateLimitEmail, &email)
        )
    })
}

fn secondary_identifier(
    endpoint: Endpoint,
    identity: &RateLimitIdentity<'_>,
    source: &str,
) -> Result<Option<String>, AuthError> {
    use RateLimitSubject as Subject;
    match (endpoint, identity.subject) {
        (
            Endpoint::PasswordRegister
            | Endpoint::PasswordLogin
            | Endpoint::PasswordResetRequest
            | Endpoint::ProviderEmailSend,
            Subject::Email(email),
        ) => Ok(email_identifier(identity.secret, email)),
        (
            Endpoint::Authorize | Endpoint::Token | Endpoint::OAuthRevoke,
            Subject::Client(client),
        ) => {
            // JSON tuple framing prevents delimiter collisions. None collapses all unknown clients.
            let framed = serde_json::to_string(&(client, source))
                .map_err(|_| unavailable(endpoint, "encode"))?;
            Ok(Some(format!(
                "client_source:{}",
                lookup_digest(
                    identity.secret,
                    LookupFamily::RateLimitClientSource,
                    &framed
                )
            )))
        }
        (
            Endpoint::ProviderAuthorize,
            Subject::Provider {
                provider,
                session_digest,
            },
        ) => {
            let framed = serde_json::to_string(&(provider, session_digest, source))
                .map_err(|_| unavailable(endpoint, "encode"))?;
            Ok(Some(format!(
                "provider_source:{}",
                lookup_digest(
                    identity.secret,
                    LookupFamily::RateLimitProviderSource,
                    &framed
                )
            )))
        }
        (endpoint, Subject::None) if !endpoint.has_secondary() => Ok(None),
        _ => Err(unavailable(endpoint, "identity_policy_mismatch")),
    }
}

pub async fn check_rate_limit<S: StorageAdapter + ?Sized>(
    storage: &S,
    config: &RateLimitConfig,
    endpoint: Endpoint,
    identity: RateLimitIdentity<'_>,
) -> Result<(), AuthError> {
    check_rate_limit_at(storage, config, endpoint, identity, Utc::now()).await
}

/// Explicit time seam shared by memory and real DynamoDB contract tests.
pub async fn check_rate_limit_at<S: StorageAdapter + ?Sized>(
    storage: &S,
    config: &RateLimitConfig,
    endpoint: Endpoint,
    identity: RateLimitIdentity<'_>,
    now: DateTime<Utc>,
) -> Result<(), AuthError> {
    if !config.enabled {
        return Ok(());
    }
    let policy = config
        .policies
        .get(&endpoint)
        .filter(|policy| policy.valid(endpoint))
        .ok_or_else(|| unavailable(endpoint, "invalid_policy"))?;
    if identity.secret.is_empty() {
        return Err(unavailable(endpoint, "missing_secret"));
    }
    let source = source_identifier(identity.secret, identity.source);
    let secondary = secondary_identifier(endpoint, &identity, &source)?;
    check_bucket(storage, endpoint, &source, &policy.source, now).await?;
    if let (Some(identifier), Some(limit)) = (secondary, &policy.secondary) {
        // No refund on denial/failure: the source already admitted this attempt.
        check_bucket(storage, endpoint, &identifier, limit, now).await?;
    }
    Ok(())
}

async fn check_bucket<S: StorageAdapter + ?Sized>(
    storage: &S,
    endpoint: Endpoint,
    identifier: &str,
    limit: &RateLimit,
    now: DateTime<Utc>,
) -> Result<(), AuthError> {
    let key = ["ratelimit", endpoint.as_str(), identifier];
    let window = Duration::seconds(limit.window_seconds as i64);
    let expiry = now + window * 2;
    for _ in 0..RATE_LIMIT_CAS_RETRIES {
        let current_value = storage
            .get(&key)
            .await
            .map_err(|_| unavailable(endpoint, "read"))?;
        let counter = current_value
            .as_ref()
            .map(|value| serde_json::from_value::<RateLimitCounter>(value.clone()))
            .transpose()
            .map_err(|_| unavailable(endpoint, "decode"))?;
        if counter.as_ref().is_some_and(|c| c.count == 0) {
            return Err(unavailable(endpoint, "invalid_counter"));
        }
        let active = counter.as_ref().filter(|c| c.window_start > now - window);
        if let Some(counter) = active.filter(|c| c.count >= limit.requests) {
            let remaining_ms = (counter.window_start + window - now).num_milliseconds();
            return Err(AuthError::RateLimitExceeded {
                limit: limit.requests,
                window_seconds: limit.window_seconds,
                retry_after: ((remaining_ms + 999) / 1000).max(1) as u64,
            });
        }
        let next = RateLimitCounter {
            count: active.map_or(1, |c| c.count + 1),
            window_start: active.map_or(now, |c| c.window_start),
        };
        let value = serde_json::to_value(next).map_err(|_| unavailable(endpoint, "encode"))?;
        if storage
            .compare_and_set(&key, current_value.as_ref(), value, Some(expiry))
            .await
            .map_err(|_| unavailable(endpoint, "write"))?
        {
            return Ok(());
        }
    }
    tracing::warn!(
        endpoint = endpoint.as_str(),
        "rate limit counter contention exhausted"
    );
    Err(AuthError::RateLimitExceeded {
        limit: limit.requests,
        window_seconds: limit.window_seconds,
        retry_after: 1,
    })
}

fn unavailable(endpoint: Endpoint, operation: &'static str) -> AuthError {
    // Storage errors can contain keys/values; report only a fixed safe operation category.
    tracing::warn!(
        endpoint = endpoint.as_str(),
        operation,
        "rate limit unavailable"
    );
    AuthError::RateLimitUnavailable
}
