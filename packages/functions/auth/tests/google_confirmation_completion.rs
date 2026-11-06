#[path = "support/sign_in_confirmation.rs"]
#[allow(dead_code, unused_imports, unused_macros)]
mod confirmation_contract;
#[path = "support/confirmation_mail.rs"]
#[allow(dead_code)]
mod confirmation_mail;
#[path = "google_confirmation_completion/protocol.rs"]
mod protocol;
#[path = "support/provider_issuance.rs"]
#[allow(dead_code, unused_imports, unused_macros)]
mod provider_contract;
#[path = "support/rate_control.rs"]
mod rate_control;
mod support;
#[path = "support/transaction_gate.rs"]
#[allow(dead_code)]
mod transaction_gate;

use confirmation_contract::Fixture;
use irongate::{
    config::{Endpoint, RateLimitConfig},
    crypto::hmac_lookup::{lookup_digest, LookupFamily},
    error::AuthError,
    providers::email_confirmation::{
        complete_google_confirmation, CompleteGoogleConfirmationInput, CompletedGoogleConfirmation,
        ConfirmationError as Error,
    },
    storage::{StorageAdapter, TransactOperation},
    store::{
        keys::StoreKey,
        rate_limits::{email_identifier, source_identifier},
        AuthStore,
    },
};
use provider_contract::{ownership::runtime, SECRET};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use transaction_gate::PausedStorage;

async fn complete(
    f: &Fixture,
    store: &AuthStore,
    limits: &RateLimitConfig,
    code: &str,
    source: &str,
) -> Result<CompletedGoogleConfirmation, Error> {
    complete_google_confirmation(
        store,
        &runtime(),
        limits,
        CompleteGoogleConfirmationInput {
            request: &f.request,
            browser_proof: &f.browser,
            code,
            trusted_source: Some(source),
        },
    )
    .await
}

fn spawn_complete(
    f: &Fixture,
    store: AuthStore,
) -> tokio::task::JoinHandle<Result<CompletedGoogleConfirmation, Error>> {
    let (request, browser, code) = (f.request.clone(), f.browser.clone(), f.proof.clone());
    tokio::spawn(async move {
        complete_google_confirmation(
            &store,
            &runtime(),
            &Default::default(),
            CompleteGoogleConfirmationInput {
                request: &request,
                browser_proof: &browser,
                code: &code,
                trusted_source: Some("203.0.113.16"),
            },
        )
        .await
    })
}

fn limits(source: u32, email: u32) -> RateLimitConfig {
    let mut limits = RateLimitConfig::default();
    limits
        .policies
        .get_mut(&Endpoint::ProviderEmailAttempt)
        .unwrap()
        .source
        .requests = source;
    limits
        .policies
        .get_mut(&Endpoint::ProviderEmailConfirm)
        .unwrap()
        .secondary
        .as_mut()
        .unwrap()
        .requests = email;
    limits
}

fn code_from_redirect(result: &CompletedGoogleConfirmation) -> String {
    url::Url::parse(&result.redirect_uri)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "code")
        .unwrap()
        .1
        .into_owned()
}

#[tokio::test]
async fn source_ceiling_stops_reads_across_changed_requests_and_clients() {
    let storage = Arc::new(support::TestStorage::new());
    let control =
        rate_control::RateControl::new(storage.clone(), "source:", rate_control::Fault::Observe);
    for n in 0..3 {
        let mut f = Fixture::new(storage.clone(), &format!("source-{n}"));
        f.provider.session.client_id = format!("client-{n}");
        f.create().await;
        let before = f.row().await;
        let result = complete(
            &f,
            &f.store(control.clone()),
            &limits(2, 10),
            "bad-code",
            "203.0.113.16",
        )
        .await;
        if n < 2 {
            assert!(matches!(result, Err(Error::InvalidConfirmation)));
        } else {
            assert!(matches!(
                result,
                Err(Error::Admission(AuthError::RateLimitExceeded {
                    limit: 2,
                    ..
                }))
            ));
        }
        assert_eq!(f.row().await, before);
    }
    assert_eq!(control.protected_reads.load(Ordering::SeqCst), 2);
    assert!(storage
        .query_prefix(&["oauth:code"])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn email_budget_survives_fresh_requests_and_sources() {
    let storage = Arc::new(support::TestStorage::new());
    for n in 0..3 {
        let mut f = Fixture::new(storage.clone(), &format!("email-{n}"));
        f.email = if n == 0 {
            " MAILBOX@Example.test "
        } else {
            "mailbox@example.test"
        }
        .into();
        f.provider.session.client_id = format!("client-{n}");
        f.create().await;
        let before = f.row().await;
        let result = complete(
            &f,
            &f.provider.store,
            &limits(10, 2),
            if n < 2 { "incorrect" } else { &f.proof },
            &format!("203.0.113.{}", n + 1),
        )
        .await;
        if n < 2 {
            assert!(matches!(result, Err(Error::InvalidConfirmation)));
        } else {
            assert!(matches!(
                result,
                Err(Error::Admission(AuthError::RateLimitExceeded {
                    limit: 2,
                    retry_after: 1..=60,
                    ..
                }))
            ));
        }
        assert_eq!(f.row().await, before);
    }
    let other = Fixture::new(storage.clone(), "independent-mailbox");
    other.create().await;
    complete(
        &other,
        &other.provider.store,
        &limits(10, 2),
        &other.proof,
        "203.0.113.4",
    )
    .await
    .unwrap();
    for (key, _) in storage.query_prefix(&["ratelimit"]).await.unwrap() {
        let key = key.join(":");
        assert!(!key.contains("203.0.113"));
        assert!(!key.contains("example.test"));
        assert!(!key.contains(&other.request));
        assert!(!key.contains(&other.browser));
    }
}

#[tokio::test]
async fn bad_browser_or_request_never_selects_an_email_budget() {
    for mode in [
        "malformed-request",
        "unknown",
        "wrong-browser",
        "malformed-browser",
        "expired",
    ] {
        let storage = Arc::new(support::TestStorage::new());
        let mut f = Fixture::new(storage.clone(), mode);
        f.create().await;
        let original_key = f.key();
        let before = f.row().await;
        match mode {
            "malformed-request" => f.request = "bad".into(),
            "unknown" => f.request = "z".repeat(32),
            "wrong-browser" => f.browser = "z".repeat(32),
            "malformed-browser" => f.browser.clear(),
            _ => *f.provider.clock.lock().unwrap() = f.provider.session.expires_at,
        }
        assert!(matches!(
            complete(
                &f,
                &f.provider.store,
                &Default::default(),
                &f.proof,
                "203.0.113.16"
            )
            .await,
            Err(Error::InvalidConfirmation)
        ));
        assert_eq!(
            storage
                .query_prefix(&["ratelimit", "provider_email_attempt"])
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(storage
            .query_prefix(&["ratelimit", "provider_email_confirm"])
            .await
            .unwrap()
            .is_empty());
        assert_eq!(f.provider.rows(&[original_key]).await[0], before);
    }
}

#[tokio::test]
async fn admission_faults_stop_issuance_without_refunds() {
    for mode in [
        "missing-source",
        "invalid-email",
        "source-read",
        "email-read",
        "source-corrupt",
        "email-corrupt",
        "lost-email",
        "disabled",
    ] {
        let storage = Arc::new(support::TestStorage::new());
        let f = Fixture::new(storage.clone(), mode);
        f.create().await;
        let before = f.row().await;
        let mut policy = limits(10, 1);
        let dimension = if mode.contains("email") {
            "email:"
        } else {
            "source:"
        };
        let fault = match mode {
            "source-read" | "email-read" | "disabled" => rate_control::Fault::Read,
            "lost-email" => rate_control::Fault::LostResponse,
            _ => rate_control::Fault::Observe,
        };
        let control = rate_control::RateControl::new(storage.clone(), dimension, fault);
        match mode {
            "missing-source" => {
                policy.policies.remove(&Endpoint::ProviderEmailAttempt);
            }
            "invalid-email" => {
                policy
                    .policies
                    .get_mut(&Endpoint::ProviderEmailConfirm)
                    .unwrap()
                    .secondary = None;
            }
            "source-corrupt" | "email-corrupt" => {
                let (endpoint, id) = if mode == "source-corrupt" {
                    (
                        "provider_email_attempt",
                        source_identifier(SECRET, Some("203.0.113.16")),
                    )
                } else {
                    (
                        "provider_email_confirm",
                        email_identifier(SECRET, &f.email).unwrap(),
                    )
                };
                storage
                    .set(
                        &["ratelimit", endpoint, &id],
                        json!({"count":0,"window_start":chrono::Utc::now()}),
                        None,
                    )
                    .await
                    .unwrap();
            }
            "disabled" => policy.enabled = false,
            _ => {}
        }
        let result = complete(
            &f,
            &f.store(control.clone()),
            &policy,
            &f.proof,
            "203.0.113.16",
        )
        .await;
        if mode == "disabled" {
            assert!(result.is_ok());
            assert_eq!(control.reads.load(Ordering::SeqCst), 0);
        } else {
            assert!(
                matches!(
                    result,
                    Err(Error::Admission(AuthError::RateLimitUnavailable))
                ),
                "{mode}"
            );
            assert_eq!(f.row().await, before);
            assert!(storage
                .query_prefix(&["oauth:code"])
                .await
                .unwrap()
                .is_empty());
            assert_eq!(
                control.protected_reads.load(Ordering::SeqCst),
                usize::from(mode.contains("email"))
            );
        }
        if mode == "lost-email" {
            assert!(matches!(
                complete(&f, &f.provider.store, &policy, &f.proof, "203.0.113.17").await,
                Err(Error::Admission(AuthError::RateLimitExceeded {
                    limit: 1,
                    ..
                }))
            ));
            assert_eq!(f.row().await, before);
        }
    }
}

#[tokio::test]
async fn code_shape_rejections_charge_email_and_keep_pending() {
    for code in [
        "",
        "1234567",
        "012345678",
        " 01234567",
        "01234567 ",
        "１２３４５６７８",
        "76543210",
    ] {
        let storage = Arc::new(support::TestStorage::new());
        let f = Fixture::new(storage.clone(), "code-shape");
        f.create().await;
        let before = f.row().await;
        assert!(matches!(
            complete(&f, &f.provider.store, &limits(10, 1), code, "203.0.113.16").await,
            Err(Error::InvalidConfirmation)
        ));
        assert_eq!(f.row().await, before);
        assert!(matches!(
            complete(
                &f,
                &f.provider.store,
                &limits(10, 1),
                &f.proof,
                "203.0.113.17"
            )
            .await,
            Err(Error::Admission(AuthError::RateLimitExceeded {
                limit: 1,
                ..
            }))
        ));
        assert_eq!(f.row().await, before);
    }
}

#[tokio::test]
async fn expiry_during_email_admission_does_not_issue() {
    let storage = Arc::new(support::TestStorage::new());
    let f = Fixture::new(storage.clone(), "expiry-admission");
    let deadline = f.create_with(&f.provider.store, 30).await.unwrap();
    let before = f.row().await;
    let id = email_identifier(SECRET, &f.email).unwrap();
    let (paused, mut gate) = PausedStorage::before_read(
        storage.clone(),
        vec!["ratelimit".into(), "provider_email_confirm".into(), id],
    );
    let task = spawn_complete(&f, f.store(paused));
    let read = gate.wait().await;
    *f.provider.clock.lock().unwrap() = deadline;
    read.commit();
    assert!(matches!(
        task.await.unwrap(),
        Err(Error::InvalidConfirmation)
    ));
    assert_eq!(f.row().await, before);
    assert!(storage
        .query_prefix(&["oauth:code"])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn malformed_pending_and_read_outages_are_not_bad_proofs() {
    for mode in ["record", "read", "redirect"] {
        let storage = Arc::new(support::TestStorage::new());
        let mut f = Fixture::new(storage.clone(), mode);
        if mode == "redirect" {
            f.provider.session.redirect_uri = "not an absolute URL".into();
        }
        f.create().await;
        if mode == "record" {
            let mut row = f.row().await.unwrap();
            row["purpose"] = json!("sensitive-marker");
            f.provider.replace(&f.key(), row).await;
        }
        let before = f.row().await;
        let result = if mode == "read" {
            let (paused, mut gate) = PausedStorage::before_read(storage.clone(), f.key().parts());
            let task = spawn_complete(&f, f.store(paused));
            gate.wait().await.fail_before_dispatch();
            task.await.unwrap()
        } else {
            complete(
                &f,
                &f.provider.store,
                &Default::default(),
                &f.proof,
                "203.0.113.16",
            )
            .await
        };
        let error = result.err().expect("must fail");
        assert!(matches!(error, Error::StorageUnavailable));
        assert!(!format!("{error:?} {error}").contains("sensitive-marker"));
        assert_eq!(f.row().await, before);
        assert!(storage
            .query_prefix(&["oauth:code"])
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn completion_failures_and_delayed_success_preserve_commit_truth() {
    for mode in ["before", "lost", "conflict", "delayed"] {
        let storage = Arc::new(support::TestStorage::new());
        let f = Fixture::new(storage.clone(), mode);
        let deadline = f.create_with(&f.provider.store, 30).await.unwrap();
        let (paused, mut gate) = PausedStorage::new(storage.clone());
        let task = spawn_complete(&f, f.store(paused));
        let tx = gate.wait().await;
        let digest = tx
            .operations
            .iter()
            .find_map(|op| match op {
                TransactOperation::Put { key, .. } if key[0] == "oauth:code" => {
                    Some(key[1].clone())
                }
                _ => None,
            })
            .unwrap();
        let keys = f.keys(&digest).await;
        if mode == "conflict" {
            let mut row = f.row().await.unwrap();
            row["session"]["state"] = json!("competing-writer");
            f.provider.replace(&f.key(), row).await;
        }
        let before = f.provider.rows(&keys).await;
        match mode {
            "before" => tx.fail_before_dispatch(),
            "lost" => tx.lose_response(),
            "conflict" => tx.commit(),
            _ => {
                let release = tx.commit_and_hold_response().await;
                assert!(f.row().await.is_none());
                *f.provider.clock.lock().unwrap() = deadline;
                release.send(()).unwrap();
            }
        }
        let result = task.await.unwrap();
        if mode == "delayed" {
            let result = result.unwrap();
            assert_eq!(
                lookup_digest(
                    SECRET,
                    LookupFamily::AuthorizationCode,
                    &code_from_redirect(&result)
                ),
                digest
            );
        } else {
            let error = result.err().expect("must not return continuation");
            assert!(if mode == "conflict" {
                matches!(error, Error::InvalidConfirmation)
            } else {
                matches!(error, Error::StorageUnavailable)
            });
        }
        if mode == "lost" || mode == "delayed" {
            assert!(f.row().await.is_none());
            let code = f
                .provider
                .store
                .get_authorization_code(&digest)
                .await
                .unwrap()
                .unwrap();
            assert!(f
                .provider
                .store
                .is_active_account(&irongate::core::subjects::Subject::from_persisted(
                    code.subject
                ))
                .await
                .unwrap());
            assert!(matches!(
                complete(
                    &f,
                    &f.provider.store,
                    &Default::default(),
                    &f.proof,
                    "203.0.113.17"
                )
                .await,
                Err(Error::InvalidConfirmation)
            ));
            assert_eq!(
                storage.query_prefix(&["oauth:code"]).await.unwrap().len(),
                1
            );
        } else {
            assert_eq!(f.provider.rows(&keys).await, before);
        }
    }
}

#[tokio::test]
async fn competing_completions_return_only_one_code() {
    let storage = Arc::new(support::TestStorage::new());
    let f = Fixture::new(storage.clone(), "competing");
    f.create().await;
    let (a, mut ag) = PausedStorage::new(storage.clone());
    let (b, mut bg) = PausedStorage::new(storage.clone());
    let aj = spawn_complete(&f, f.store(a));
    let a = ag.wait().await;
    let bj = spawn_complete(&f, f.store(b));
    let b = bg.wait().await;
    a.commit();
    let winner = aj.await.unwrap().unwrap();
    b.commit();
    assert!(matches!(bj.await.unwrap(), Err(Error::InvalidConfirmation)));
    let digest = lookup_digest(
        SECRET,
        LookupFamily::AuthorizationCode,
        &code_from_redirect(&winner),
    );
    let codes = storage.query_prefix(&["oauth:code"]).await.unwrap();
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0].0, StoreKey::authorization_code(&digest).parts());
    assert!(f.row().await.is_none());
}
