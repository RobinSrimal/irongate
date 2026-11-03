//! Admission cases shared by memory and real DynamoDB Local.
use super::rate_control::{Fault, RateControl};
use chrono::{DateTime, Duration, Utc};
use irongate::{
    config::{Endpoint, RateLimitConfig},
    error::AuthError,
    storage::StorageAdapter,
    store::rate_limits::{
        check_rate_limit_at, email_identifier, source_identifier, RateLimitIdentity,
        RateLimitSubject,
    },
};
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Arc};

pub const SECRET: &[u8] = b"test-only-rate-secret";
pub const ENDPOINT: Endpoint = Endpoint::PasswordLogin;
pub const IP: &str = "203.0.113.1";
pub const EMAIL: &str = "user@example.com";

pub fn config(source: u32, email: u32) -> RateLimitConfig {
    let mut config = RateLimitConfig::default();
    let policy = config.policies.get_mut(&ENDPOINT).unwrap();
    policy.source.requests = source;
    policy.secondary.as_mut().unwrap().requests = email;
    config
}
pub async fn check(
    storage: &dyn StorageAdapter,
    config: &RateLimitConfig,
    now: DateTime<Utc>,
    source: &str,
    email: &str,
) -> Result<(), AuthError> {
    check_rate_limit_at(
        storage,
        config,
        ENDPOINT,
        RateLimitIdentity {
            secret: SECRET,
            source: Some(source),
            subject: RateLimitSubject::Email(email),
        },
        now,
    )
    .await
}
async fn rows(storage: &dyn StorageAdapter) -> Vec<(Vec<String>, Value)> {
    storage
        .query_prefix(&["ratelimit", ENDPOINT.as_str(), ""])
        .await
        .unwrap()
}
async fn clear(storage: &dyn StorageAdapter) {
    for (key, _) in rows(storage).await {
        storage
            .remove(&key.iter().map(String::as_str).collect::<Vec<_>>())
            .await
            .unwrap();
    }
}
fn limited(result: Result<(), AuthError>) {
    assert!(matches!(
        result,
        Err(AuthError::RateLimitExceeded {
            retry_after: 1..,
            ..
        })
    ));
}

pub async fn exact_window(storage: Arc<dyn StorageAdapter>) {
    let now = Utc::now();
    check(storage.as_ref(), &config(10, 1), now, IP, EMAIL)
        .await
        .unwrap();
    assert!(matches!(
        check(
            storage.as_ref(),
            &config(10, 1),
            now + Duration::milliseconds(59_001),
            IP,
            EMAIL
        )
        .await,
        Err(AuthError::RateLimitExceeded { retry_after: 1, .. })
    ));
    let key = email_identifier(SECRET, EMAIL).unwrap();
    assert!(storage
        .get(&["ratelimit", ENDPOINT.as_str(), &key])
        .await
        .unwrap()
        .is_some());
    check(
        storage.as_ref(),
        &config(10, 1),
        now + Duration::seconds(60),
        IP,
        EMAIL,
    )
    .await
    .unwrap();
    let records = rows(storage.as_ref()).await;
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|(_, value)| value["count"] == 1));
    // Only limiter time moved. The backing store's clock and TTL cleanup did not.
}

pub async fn concurrent_dimensions(storage: Arc<dyn StorageAdapter>) {
    for dimension in ["source:", "email:"] {
        clear(storage.as_ref()).await;
        let config = Arc::new(if dimension == "source:" {
            config(2, 50)
        } else {
            config(50, 2)
        });
        let control = RateControl::new(storage.clone(), dimension, Fault::Race(12));
        let now = Utc::now();
        let mut tasks = Vec::new();
        for n in 0..12 {
            let (control, config) = (control.clone(), config.clone());
            tasks.push(tokio::spawn(async move {
                let ip = if dimension == "source:" {
                    IP.into()
                } else {
                    format!("203.0.113.{}", n + 10)
                };
                let email = if dimension == "source:" {
                    format!("user{n}@example.com")
                } else {
                    EMAIL.into()
                };
                check(control.as_ref(), &config, now, &ip, &email).await
            }));
        }
        let mut allowed = 0;
        for task in tasks {
            match task.await.unwrap() {
                Ok(()) => allowed += 1,
                result => limited(result),
            }
        }
        assert_eq!(allowed, 2);
        let records = rows(storage.as_ref()).await;
        assert!(!records.is_empty());
        for (key, value) in &records {
            if key[2].starts_with(dimension) {
                assert!(value["count"].as_u64().unwrap() <= 2);
            }
        }
        if dimension == "source:" {
            assert_eq!(records.len(), 3, "denied source allocates no email rows");
        }
    }
}

pub async fn failures_and_accounting(storage: Arc<dyn StorageAdapter>) {
    let now = Utc::now();
    for dimension in ["source:", "email:"] {
        for fault in [Fault::Read, Fault::Write, Fault::LostResponse] {
            clear(storage.as_ref()).await;
            let committed = matches!(fault, Fault::LostResponse);
            let write_attempt = !matches!(fault, Fault::Read);
            let control = RateControl::new(storage.clone(), dimension, fault);
            assert!(matches!(
                check(control.as_ref(), &config(10, 1), now, IP, EMAIL).await,
                Err(AuthError::RateLimitUnavailable)
            ));
            assert_eq!(control.reads.load(Ordering::SeqCst), 1);
            assert_eq!(
                control.writes.load(Ordering::SeqCst),
                usize::from(write_attempt)
            );
            let records = rows(storage.as_ref()).await;
            assert_eq!(
                records.len(),
                usize::from(dimension == "email:") + usize::from(committed)
            );
            assert!(records.iter().all(|(_, value)| value["count"] == 1));
        }
    }
    clear(storage.as_ref()).await;
    check(storage.as_ref(), &config(2, 1), now, IP, EMAIL)
        .await
        .unwrap();
    limited(check(storage.as_ref(), &config(2, 1), now, IP, EMAIL).await);
    limited(
        check(
            storage.as_ref(),
            &config(2, 1),
            now,
            IP,
            "other@example.com",
        )
        .await,
    );
    let source = source_identifier(SECRET, Some(IP));
    let records = rows(storage.as_ref()).await;
    assert_eq!(
        records.len(),
        2,
        "no refund or extra secondary row after denial"
    );
    assert_eq!(
        records.iter().find(|(key, _)| key[2] == source).unwrap().1["count"],
        2
    );
}

pub async fn contention_and_corrupt_counters(storage: Arc<dyn StorageAdapter>) {
    let now = Utc::now();
    for dimension in ["source:", "email:"] {
        clear(storage.as_ref()).await;
        let control = RateControl::new(storage.clone(), dimension, Fault::Conflict);
        assert!(matches!(
            check(control.as_ref(), &config(10, 5), now, IP, EMAIL).await,
            Err(AuthError::RateLimitExceeded { retry_after: 1, .. })
        ));
        assert_eq!(control.reads.load(Ordering::SeqCst), 8);
        assert_eq!(control.writes.load(Ordering::SeqCst), 8);
        assert_eq!(
            rows(storage.as_ref()).await.len(),
            usize::from(dimension == "email:")
        );
    }
    for value in [
        json!({"bad": "counter"}),
        json!({"count": 0, "window_start": now}),
    ] {
        clear(storage.as_ref()).await;
        let id = email_identifier(SECRET, EMAIL).unwrap();
        storage
            .set(
                &["ratelimit", ENDPOINT.as_str(), &id],
                value.clone(),
                Some(now + Duration::seconds(120)),
            )
            .await
            .unwrap();
        assert!(matches!(
            check(storage.as_ref(), &config(10, 5), now, IP, EMAIL).await,
            Err(AuthError::RateLimitUnavailable)
        ));
        assert_eq!(
            storage
                .get(&["ratelimit", ENDPOINT.as_str(), &id])
                .await
                .unwrap(),
            Some(value)
        );
        assert_eq!(
            rows(storage.as_ref()).await.len(),
            2,
            "source charge remains"
        );
    }
}

pub async fn invalid_policy(storage: Arc<dyn StorageAdapter>) {
    let now = Utc::now();
    let mut variants = Vec::new();
    let mut missing = config(1, 1);
    missing.policies.remove(&ENDPOINT);
    variants.push(missing);
    for (requests, window) in [(0, 60), (1_000_001, 60), (1, 0), (1, 86_401), (1, u64::MAX)] {
        for secondary in [false, true] {
            let mut policy = config(1, 1);
            let limits = policy.policies.get_mut(&ENDPOINT).unwrap();
            let limit = if secondary {
                limits.secondary.as_mut().unwrap()
            } else {
                &mut limits.source
            };
            limit.requests = requests;
            limit.window_seconds = window;
            variants.push(policy);
        }
    }
    let mut missing_email = config(1, 1);
    missing_email.policies.get_mut(&ENDPOINT).unwrap().secondary = None;
    variants.push(missing_email);
    let control = RateControl::new(storage.clone(), "", Fault::Observe);
    for mut policy in variants {
        assert!(matches!(
            check(control.as_ref(), &policy, now, IP, EMAIL).await,
            Err(AuthError::RateLimitUnavailable)
        ));
        policy.enabled = false;
        check(control.as_ref(), &policy, now, IP, EMAIL)
            .await
            .unwrap();
    }
    for (endpoint, subject, secret) in [
        (ENDPOINT, RateLimitSubject::None, SECRET),
        (
            Endpoint::PasswordVerify,
            RateLimitSubject::Email(EMAIL),
            SECRET,
        ),
        (ENDPOINT, RateLimitSubject::Email(EMAIL), b"".as_slice()),
    ] {
        assert!(matches!(
            check_rate_limit_at(
                control.as_ref(),
                &config(1, 1),
                endpoint,
                RateLimitIdentity {
                    secret,
                    source: Some(IP),
                    subject
                },
                now
            )
            .await,
            Err(AuthError::RateLimitUnavailable)
        ));
    }
    assert_eq!(control.reads.load(Ordering::SeqCst), 0);
    assert_eq!(control.writes.load(Ordering::SeqCst), 0);
    assert!(rows(storage.as_ref()).await.is_empty());
}

pub async fn private_bounded_keys(storage: Arc<dyn StorageAdapter>) {
    assert_eq!(
        email_identifier(SECRET, " USER@EXAMPLE.COM "),
        email_identifier(SECRET, EMAIL)
    );
    assert_ne!(
        email_identifier(SECRET, EMAIL),
        email_identifier(b"another-secret", EMAIL)
    );
    assert_ne!(
        source_identifier(SECRET, Some(IP)),
        source_identifier(b"another-secret", Some(IP))
    );
    assert_eq!(
        source_identifier(SECRET, Some("2001:0DB8:0:0:0:0:0:1")),
        source_identifier(SECRET, Some("2001:db8::1"))
    );
    assert_eq!(
        source_identifier(SECRET, Some("::ffff:203.0.113.1")),
        source_identifier(SECRET, Some(IP))
    );
    for invalid in ["", " ", "not-an-ip", "203.0.113.1:80"] {
        assert_eq!(
            source_identifier(SECRET, Some(invalid)),
            source_identifier(SECRET, None)
        );
    }
    let now = Utc::now();
    for email in ["bad", "no-at", "a@b"] {
        check(storage.as_ref(), &config(3, 1), now, IP, email)
            .await
            .unwrap();
    }
    limited(check(storage.as_ref(), &config(3, 1), now, IP, EMAIL).await);
    let records = rows(storage.as_ref()).await;
    assert_eq!(records.len(), 1);
    let text = serde_json::to_string(&records).unwrap();
    assert!(!text.contains(IP) && !text.contains(EMAIL));
    assert_eq!(records[0].0[2].len(), "source:".len() + 64);
}

macro_rules! rate_cases {
    ($runner:path $(, $attribute:meta)?) => {
        #[tokio::test] $(#[$attribute])? async fn rate_exact_window() { $runner(rate_contract::exact_window).await; }
        #[tokio::test] $(#[$attribute])? async fn rate_concurrent_dimensions() { $runner(rate_contract::concurrent_dimensions).await; }
        #[tokio::test] $(#[$attribute])? async fn rate_failures_and_accounting() { $runner(rate_contract::failures_and_accounting).await; }
        #[tokio::test] $(#[$attribute])? async fn rate_contention_and_corrupt_counters() { $runner(rate_contract::contention_and_corrupt_counters).await; }
        #[tokio::test] $(#[$attribute])? async fn rate_invalid_policy() { $runner(rate_contract::invalid_policy).await; }
        #[tokio::test] $(#[$attribute])? async fn rate_private_keys() { $runner(rate_contract::private_bounded_keys).await; }
    };
}
pub(crate) use rate_cases;
