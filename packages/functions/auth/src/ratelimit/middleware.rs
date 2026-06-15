//! Rate limiting middleware.
//!
//! DynamoDB-based rate limiting with sliding window.

use axum::http::{Extensions, HeaderMap};
use chrono::{Duration, Utc};
use lambda_http::request::RequestContext;
use serde::{Deserialize, Serialize};

use crate::config::{Endpoint, RateLimitConfig};
use crate::error::AuthError;
use crate::storage::StorageAdapter;

/// Rate limit counter stored in DynamoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitCounter {
    pub count: u32,
    pub window_start: chrono::DateTime<Utc>,
}

/// Check rate limit for an endpoint.
///
/// Returns Ok(()) if within limits, Err with retry info if exceeded.
pub async fn check_rate_limit<S: StorageAdapter>(
    storage: &S,
    config: &RateLimitConfig,
    endpoint: Endpoint,
    identifier: &str, // IP address or client_id
) -> Result<(), AuthError> {
    if !config.enabled {
        return Ok(());
    }

    let limit = match config.limits.get(&endpoint) {
        Some(l) => l,
        None => return Ok(()), // No limit configured for this endpoint
    };

    let key = ["ratelimit", endpoint.as_str(), identifier];
    let now = Utc::now();
    let window_start = now - Duration::seconds(limit.window_seconds as i64);

    // Get current count
    let current: Option<RateLimitCounter> = storage
        .get(&key.iter().map(|s| *s).collect::<Vec<_>>())
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok());

    let count = match current {
        Some(counter) if counter.window_start > window_start => counter.count + 1,
        _ => 1,
    };

    if count > limit.requests {
        return Err(AuthError::RateLimitExceeded {
            limit: limit.requests,
            window_seconds: limit.window_seconds,
            retry_after: limit.window_seconds,
        });
    }

    // Update counter
    let new_counter = RateLimitCounter {
        count,
        window_start: now,
    };

    let expiry = now + Duration::seconds(limit.window_seconds as i64 * 2);
    let _ = storage
        .set(
            &key.iter().map(|s| *s).collect::<Vec<_>>(),
            serde_json::to_value(&new_counter).unwrap(),
            Some(expiry),
        )
        .await;

    Ok(())
}

/// Get the client identifier for rate limiting.
///
/// Uses client_id if available, otherwise falls back to IP address.
pub fn get_rate_limit_identifier(client_id: Option<&str>, ip_address: Option<&str>) -> String {
    client_id
        .or(ip_address)
        .unwrap_or("unknown")
        .to_string()
}

/// Extract trusted source IP from Lambda/API Gateway request context.
///
/// Request headers such as x-forwarded-for and x-real-ip are intentionally
/// ignored here because API Gateway mode should anchor source identity in the
/// request context produced by AWS.
pub fn trusted_source_ip(extensions: &Extensions, _headers: &HeaderMap) -> Option<String> {
    extensions
        .get::<RequestContext>()
        .and_then(trusted_source_ip_from_context)
}

pub fn trusted_source_ip_from_context(context: &RequestContext) -> Option<String> {
    match context {
        RequestContext::ApiGatewayV1(context) => context.identity.source_ip.clone(),
        RequestContext::ApiGatewayV2(context) => context.http.source_ip.clone(),
        _ => None,
    }
    .and_then(non_empty_trimmed)
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_identifier_uses_client_before_ip() {
        assert_eq!(
            get_rate_limit_identifier(Some("web"), Some("203.0.113.10")),
            "web"
        );
    }
}
