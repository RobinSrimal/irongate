//! Rate-limit key helpers for auth flows.

use crate::config::{Endpoint, RateLimitConfig};
use crate::core::passwords::normalize_email;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use crate::error::{AuthError, StorageError};
use crate::storage::StorageAdapter;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

const RATE_LIMIT_CAS_RETRIES: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitCounter {
    pub count: u32,
    pub window_start: chrono::DateTime<Utc>,
}

pub fn password_email_rate_limit_identifier(
    lookup_secret: &[u8],
    email: &str,
    source: Option<&str>,
) -> String {
    let email_part = normalize_email(email).ok().map(|normalized| {
        let digest = lookup_digest(lookup_secret, LookupFamily::Email, &normalized);
        format!("email:{digest}")
    });
    composite_rate_limit_identifier(email_part.as_deref(), source)
}

pub fn source_rate_limit_identifier(source: Option<&str>) -> String {
    composite_rate_limit_identifier(None, source)
}

pub fn client_source_rate_limit_identifier(
    client_id: Option<&str>,
    source: Option<&str>,
) -> String {
    let client_part = client_id.map(|client_id| format!("client:{client_id}"));
    composite_rate_limit_identifier(client_part.as_deref(), source)
}

pub fn provider_authorize_rate_limit_identifier(
    provider: &str,
    session_digest: Option<&str>,
    source: Option<&str>,
) -> String {
    let provider_part = match session_digest {
        Some(session_digest) => format!("provider:{provider}:session:{session_digest}"),
        None => format!("provider:{provider}"),
    };
    composite_rate_limit_identifier(Some(provider_part.as_str()), source)
}

pub async fn check_rate_limit<S: StorageAdapter + ?Sized>(
    storage: &S,
    config: &RateLimitConfig,
    endpoint: Endpoint,
    identifier: &str,
) -> Result<(), AuthError> {
    if !config.enabled {
        return Ok(());
    }

    let limit = match config.limits.get(&endpoint) {
        Some(limit) => limit,
        None => return Ok(()),
    };

    let key = ["ratelimit", endpoint.as_str(), identifier];
    let now = Utc::now();
    let window_cutoff = now - Duration::seconds(limit.window_seconds as i64);
    let expiry = now + Duration::seconds(limit.window_seconds as i64 * 2);

    for _ in 0..RATE_LIMIT_CAS_RETRIES {
        let current_value = storage
            .get(&key)
            .await
            .map_err(|err| rate_limit_unavailable(endpoint, identifier, err))?;
        let current_counter = current_value
            .as_ref()
            .map(|value| serde_json::from_value::<RateLimitCounter>(value.clone()))
            .transpose()
            .map_err(|err| {
                rate_limit_unavailable(
                    endpoint,
                    identifier,
                    StorageError::DynamoDB(format!("invalid rate-limit counter: {err}")),
                )
            })?;
        let active_counter = current_counter
            .as_ref()
            .filter(|counter| counter.window_start > window_cutoff);
        let count = active_counter.map_or(1, |counter| counter.count.saturating_add(1));

        if count > limit.requests {
            return Err(AuthError::RateLimitExceeded {
                limit: limit.requests,
                window_seconds: limit.window_seconds,
                retry_after: limit.window_seconds,
            });
        }

        let new_counter = RateLimitCounter {
            count,
            window_start: active_counter
                .map(|counter| counter.window_start)
                .unwrap_or(now),
        };
        let new_value = serde_json::to_value(&new_counter).map_err(|err| {
            rate_limit_unavailable(
                endpoint,
                identifier,
                StorageError::DynamoDB(format!("serialize rate-limit counter: {err}")),
            )
        })?;
        let expected = current_value.as_ref();

        let updated = storage
            .compare_and_set(&key, expected, new_value, Some(expiry))
            .await
            .map_err(|err| rate_limit_unavailable(endpoint, identifier, err))?;
        if updated {
            return Ok(());
        }
    }

    tracing::warn!(
        endpoint = endpoint.as_str(),
        identifier = %identifier,
        "rate limit counter update conflicted too many times"
    );
    Err(AuthError::RateLimitExceeded {
        limit: limit.requests,
        window_seconds: limit.window_seconds,
        retry_after: 1,
    })
}

fn composite_rate_limit_identifier(digest_part: Option<&str>, source: Option<&str>) -> String {
    let source_part = source.unwrap_or("unknown");
    match digest_part {
        Some(digest) => format!("{digest}:source:{source_part}"),
        None => format!("source:{source_part}"),
    }
}

fn rate_limit_unavailable(endpoint: Endpoint, identifier: &str, err: StorageError) -> AuthError {
    tracing::warn!(
        endpoint = endpoint.as_str(),
        identifier = %identifier,
        error = %err,
        "rate limit storage operation failed"
    );
    AuthError::RateLimitUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RateLimit;
    use crate::storage::{test_support::TestStorage, TransactOperation};
    use async_trait::async_trait;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[tokio::test]
    async fn concurrent_checks_allow_only_the_configured_bucket_count() {
        let storage = Arc::new(TestStorage::new());
        let mut limits = HashMap::new();
        limits.insert(
            Endpoint::Token,
            RateLimit {
                requests: 1,
                window_seconds: 60,
            },
        );
        let config = Arc::new(RateLimitConfig {
            enabled: true,
            limits,
        });

        let mut tasks = Vec::new();
        for _ in 0..10 {
            let storage = storage.clone();
            let config = config.clone();
            tasks.push(tokio::spawn(async move {
                check_rate_limit(
                    storage.as_ref(),
                    &config,
                    Endpoint::Token,
                    "client:web:source:test",
                )
                .await
            }));
        }

        let mut allowed = 0;
        let mut limited = 0;
        for task in tasks {
            match task.await.expect("rate-limit task") {
                Ok(()) => allowed += 1,
                Err(AuthError::RateLimitExceeded { .. }) => limited += 1,
                Err(other) => panic!("unexpected limiter result: {other:?}"),
            }
        }

        assert_eq!(allowed, 1);
        assert_eq!(limited, 9);
    }

    #[tokio::test]
    async fn stale_window_counter_is_replaced_before_ttl_expiry() {
        let storage = TestStorage::new();
        let mut limits = HashMap::new();
        limits.insert(
            Endpoint::Authorize,
            RateLimit {
                requests: 100,
                window_seconds: 60,
            },
        );
        let config = RateLimitConfig {
            enabled: true,
            limits,
        };
        let identifier = "client:web:source:test";
        let key = ["ratelimit", Endpoint::Authorize.as_str(), identifier];
        let now = Utc::now();
        let stale_counter = RateLimitCounter {
            count: 100,
            window_start: now - Duration::seconds(61),
        };

        storage
            .set(
                &key,
                serde_json::to_value(stale_counter).expect("serialize stale counter"),
                Some(now + Duration::seconds(60)),
            )
            .await
            .expect("seed stale counter");

        check_rate_limit(&storage, &config, Endpoint::Authorize, identifier)
            .await
            .expect("stale window should reset");

        let value = storage
            .get(&key)
            .await
            .expect("load reset counter")
            .expect("reset counter exists");
        let counter: RateLimitCounter =
            serde_json::from_value(value).expect("deserialize reset counter");

        assert_eq!(counter.count, 1);
        assert!(counter.window_start > now - Duration::seconds(1));
    }

    #[tokio::test]
    async fn persistent_counter_contention_returns_rate_limited_not_unavailable() {
        let storage = AlwaysConflictingStorage;
        let mut limits = HashMap::new();
        limits.insert(
            Endpoint::Authorize,
            RateLimit {
                requests: 100,
                window_seconds: 60,
            },
        );
        let config = RateLimitConfig {
            enabled: true,
            limits,
        };

        let result =
            check_rate_limit(&storage, &config, Endpoint::Authorize, "client:web:source:test")
                .await;

        assert!(matches!(
            result,
            Err(AuthError::RateLimitExceeded {
                limit: 100,
                window_seconds: 60,
                retry_after: 1,
            })
        ));
    }

    struct AlwaysConflictingStorage;

    #[async_trait]
    impl StorageAdapter for AlwaysConflictingStorage {
        async fn get(&self, _key: &[&str]) -> Result<Option<Value>, StorageError> {
            Ok(None)
        }

        async fn set(
            &self,
            _key: &[&str],
            _value: Value,
            _expiry: Option<chrono::DateTime<Utc>>,
        ) -> Result<(), StorageError> {
            unreachable!("rate-limit contention test should not call set")
        }

        async fn remove(&self, _key: &[&str]) -> Result<(), StorageError> {
            unreachable!("rate-limit contention test should not call remove")
        }

        async fn query_prefix(
            &self,
            _prefix: &[&str],
        ) -> Result<Vec<(Vec<String>, Value)>, StorageError> {
            unreachable!("rate-limit contention test should not call query_prefix")
        }

        async fn query_prefix_page(
            &self,
            _prefix: &[&str],
            _limit: u32,
            _cursor: Option<&str>,
        ) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError> {
            unreachable!("rate-limit contention test should not call query_prefix_page")
        }

        async fn compare_and_set(
            &self,
            _key: &[&str],
            _expected: Option<&Value>,
            _new_value: Value,
            _expiry: Option<chrono::DateTime<Utc>>,
        ) -> Result<bool, StorageError> {
            Ok(false)
        }

        async fn transact(&self, _operations: Vec<TransactOperation>) -> Result<(), StorageError> {
            unreachable!("rate-limit contention test should not call transact")
        }
    }
}
