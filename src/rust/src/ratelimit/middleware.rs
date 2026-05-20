//! Rate limiting middleware.
//!
//! DynamoDB-based rate limiting with sliding window.

use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{Endpoint, ProxyConfig, RateLimitConfig, TrustedProxies};
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

/// Extract client IP address from headers based on trusted proxy config.
///
/// Only trusts forwarded headers when proxies are explicitly trusted.
pub fn extract_client_ip(headers: &HeaderMap, proxy: &ProxyConfig) -> Option<String> {
    match proxy.trusted_proxies {
        TrustedProxies::None => None,
        TrustedProxies::ApiGateway | TrustedProxies::IpRanges(_) => {
            if let Some(forwarded) = headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
            {
                if let Some(first) = forwarded.split(',').next() {
                    let ip = first.trim();
                    if !ip.is_empty() {
                        return Some(ip.to_string());
                    }
                }
            }

            if let Some(real_ip) = headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
            {
                let ip = real_ip.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }

            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn extract_ip_ignores_headers_when_untrusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());

        let proxy = ProxyConfig {
            trusted_proxies: TrustedProxies::None,
        };

        assert_eq!(extract_client_ip(&headers, &proxy), None);
    }

    #[test]
    fn extract_ip_uses_forwarded_for_when_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());

        let proxy = ProxyConfig {
            trusted_proxies: TrustedProxies::ApiGateway,
        };

        assert_eq!(extract_client_ip(&headers, &proxy), Some("1.2.3.4".to_string()));
    }

    #[test]
    fn extract_ip_falls_back_to_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "9.9.9.9".parse().unwrap());

        let proxy = ProxyConfig {
            trusted_proxies: TrustedProxies::ApiGateway,
        };

        assert_eq!(extract_client_ip(&headers, &proxy), Some("9.9.9.9".to_string()));
    }
}
