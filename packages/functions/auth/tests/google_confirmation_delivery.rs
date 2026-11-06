#[path = "support/sign_in_confirmation.rs"]
#[allow(dead_code, unused_imports, unused_macros)]
mod confirmation_contract;
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
    config::{environment::RuntimeAuthConfig, Endpoint, RateLimitConfig},
    email::{EmailDeliveryError, RenderedEmail, VerificationEmailSender},
    error::{AuthError, StorageError},
    providers::email_confirmation::{
        begin_google_confirmation, ConfirmationDeliveryError as Error, GoogleConfirmationInput,
        PendingGoogleConfirmation,
    },
    storage::{StorageAdapter, TransactOperation},
    store::{sign_in_confirmation::ConfirmationDigests, AuthStore},
};
use provider_contract::{code_digest, ownership::runtime, SECRET};
use serde_json::Value;
use std::sync::{atomic::Ordering, Arc, Mutex};
use tokio::sync::Notify;

#[derive(Default)]
struct Mail {
    messages: Mutex<Vec<(String, RenderedEmail)>>,
    fail: bool,
    gate: Option<(Arc<Notify>, Arc<Notify>)>,
}
#[async_trait::async_trait]
impl VerificationEmailSender for Mail {
    async fn send_verification_email(
        &self,
        to: &str,
        message: RenderedEmail,
    ) -> Result<String, EmailDeliveryError> {
        self.messages.lock().unwrap().push((to.into(), message));
        if let Some((entered, release)) = &self.gate {
            entered.notify_one();
            tokio::time::timeout(std::time::Duration::from_secs(5), release.notified())
                .await
                .expect("release delivery");
        }
        if self.fail {
            Err(EmailDeliveryError::ProviderStatus { status: 503 })
        } else {
            Ok("test-delivery-id-not-a-continuation".into())
        }
    }
}
impl Mail {
    fn len(&self) -> usize {
        self.messages.lock().unwrap().len()
    }
    fn message(&self) -> (String, RenderedEmail) {
        self.messages.lock().unwrap().last().unwrap().clone()
    }
    fn code(&self) -> String {
        let (_, message) = self.message();
        let codes: Vec<_> = message
            .text
            .lines()
            .filter(|line| line.len() == 8 && line.bytes().all(|b| b.is_ascii_digit()))
            .collect();
        assert_eq!(codes.len(), 1);
        codes[0].into()
    }
}

async fn begin(
    f: &Fixture,
    store: &AuthStore,
    runtime: &RuntimeAuthConfig,
    limits: &RateLimitConfig,
    mail: &Mail,
    source: &str,
) -> Result<PendingGoogleConfirmation, Error> {
    begin_google_confirmation(
        store,
        runtime,
        limits,
        mail,
        GoogleConfirmationInput {
            candidate: f.candidate(),
            session: &f.provider.session,
            browser_proof: &f.browser,
            trusted_source: Some(source),
        },
    )
    .await
}
fn spawn_begin(
    f: &Fixture,
    store: AuthStore,
    mail: Arc<Mail>,
    limits: RateLimitConfig,
) -> tokio::task::JoinHandle<Result<PendingGoogleConfirmation, Error>> {
    let candidate = f.candidate();
    let session = f.provider.session.clone();
    let browser = f.browser.clone();
    tokio::spawn(async move {
        begin_google_confirmation(
            &store,
            &runtime(),
            &limits,
            mail.as_ref(),
            GoogleConfirmationInput {
                candidate,
                session: &session,
                browser_proof: &browser,
                trusted_source: Some("203.0.113.15"),
            },
        )
        .await
    })
}
async fn pending(storage: &dyn StorageAdapter) -> Vec<(Vec<String>, Value)> {
    storage
        .query_prefix(&["oauth:email_confirmation"])
        .await
        .unwrap()
}
fn limited(source: u32, email: u32) -> RateLimitConfig {
    let mut config = RateLimitConfig::default();
    let policy = config
        .policies
        .get_mut(&Endpoint::ProviderEmailSend)
        .unwrap();
    policy.source.requests = source;
    policy.secondary.as_mut().unwrap().requests = email;
    config
}

#[tokio::test]
async fn delivered_proof_completes_only_the_original_browser_and_session() {
    for seconds in [30, 1200] {
        let storage = Arc::new(support::TestStorage::new());
        let mut f = Fixture::new(storage.clone(), "delivery");
        let start = *f.provider.clock.lock().unwrap();
        f.provider.session.expires_at = start + chrono::Duration::seconds(seconds);
        let runtime = runtime();
        let mail = Mail::default();
        let result = begin(
            &f,
            &f.provider.store,
            &runtime,
            &Default::default(),
            &mail,
            "203.0.113.15",
        )
        .await
        .unwrap();
        assert_eq!(mail.len(), 1);
        assert_eq!(
            result.expires_at,
            f.provider
                .session
                .expires_at
                .min(start + chrono::Duration::seconds(runtime.ttls.provider_state_seconds as i64))
        );
        f.request = result.request;
        f.proof = mail.code();
        let (to, message) = mail.message();
        assert_eq!(to, f.email);
        assert!(message.text.contains(
            &result
                .expires_at
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        ));
        assert!(!message.text.contains(&f.request));
        assert!(!message.html.contains(&f.browser));
        let row = f.row().await.unwrap();
        assert_eq!(row["expires_at"], serde_json::json!(result.expires_at));
        for secret in [&f.request, &f.browser, &f.proof] {
            assert!(!row.to_string().contains(secret));
            assert!(!f.key().sk().contains(secret));
        }
        let code = code_digest();
        let before = f.provider.rows(&f.keys(&code).await).await;
        assert!(before[..6].iter().all(Option::is_none));
        let reconstructed = f.store(storage);
        for (request, browser, proof) in [
            (f.request.clone(), "z".repeat(32), f.proof.clone()),
            ("z".repeat(32), f.browser.clone(), f.proof.clone()),
            (
                f.request.clone(),
                f.browser.clone(),
                format!("{}x", f.proof),
            ),
        ] {
            let digests =
                ConfirmationDigests::from_secrets(SECRET, &request, &browser, &proof).unwrap();
            assert!(reconstructed
                .prepare_sign_in_confirmation(&digests, SECRET)
                .await
                .unwrap()
                .is_none());
        }
        assert_eq!(f.row().await, Some(row));
        let prepared = f.prepare(&reconstructed).await;
        let subject = reconstructed
            .finish_sign_in_confirmation(prepared, &code, 300)
            .await
            .unwrap();
        f.provider
            .assert_committed(
                &subject,
                &code,
                &serde_json::json!({"provider":"google", "email":f.email, "email_verified":true}),
                0,
            )
            .await;
        assert!(reconstructed
            .prepare_sign_in_confirmation(&f.digests(), SECRET)
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn independent_send_limits_survive_rotated_sources_and_candidate_addresses() {
    for same_source in [true, false] {
        let storage = Arc::new(support::TestStorage::new());
        let mail = Mail::default();
        let limits = if same_source {
            limited(2, 10)
        } else {
            limited(10, 2)
        };
        for attempt in 0..3 {
            let mut f = Fixture::new(storage.clone(), &format!("candidate-{attempt}"));
            f.provider.session.state = Some(format!("state-{attempt}"));
            f.provider.session.client_id = format!("client-{attempt}");
            let source = if same_source {
                "203.0.113.15".into()
            } else {
                format!("203.0.113.{}", attempt + 1)
            };
            if !same_source {
                f.email = if attempt % 2 == 0 {
                    " SAME@Example.test ".into()
                } else {
                    "same@example.test".into()
                };
            }
            let result = begin(&f, &f.provider.store, &runtime(), &limits, &mail, &source).await;
            if attempt < 2 {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(Error::Admission(AuthError::RateLimitExceeded {
                        limit: 2,
                        window_seconds: 60,
                        retry_after: 1..=60
                    }))
                ));
            }
        }
        assert_eq!(mail.len(), 2);
        assert_eq!(pending(storage.as_ref()).await.len(), 2);
        let counters = storage
            .query_prefix(&["ratelimit", "provider_email_send"])
            .await
            .unwrap();
        for (key, _) in counters {
            let key = key.join(":");
            assert!(!key.contains("203.0.113"));
            assert!(!key.contains("example.test"));
            assert!(!key.contains("candidate-"));
        }
    }
}

#[tokio::test]
async fn admission_failure_stops_protected_work_and_explicit_disable_is_preserved() {
    for mode in ["missing", "invalid", "counter", "disabled"] {
        let storage = Arc::new(support::TestStorage::new());
        let f = Fixture::new(storage.clone(), mode);
        let control =
            rate_control::RateControl::new(storage.clone(), "source:", rate_control::Fault::Read);
        let store = if mode == "counter" || mode == "disabled" {
            f.store(control.clone())
        } else {
            f.provider.store.clone()
        };
        let mut limits = RateLimitConfig::default();
        match mode {
            "missing" => {
                limits.policies.remove(&Endpoint::ProviderEmailSend);
            }
            "invalid" => {
                limits
                    .policies
                    .get_mut(&Endpoint::ProviderEmailSend)
                    .unwrap()
                    .secondary = None;
            }
            "disabled" => {
                limits.enabled = false;
            }
            _ => {}
        }
        let mail = Mail::default();
        let result = begin(&f, &store, &runtime(), &limits, &mail, "203.0.113.15").await;
        if mode == "disabled" {
            assert!(result.is_ok());
            assert_eq!(mail.len(), 1);
            assert_eq!(control.reads.load(Ordering::SeqCst), 0);
        } else {
            assert!(matches!(
                result,
                Err(Error::Admission(AuthError::RateLimitUnavailable))
            ));
            assert_eq!(mail.len(), 0);
            assert!(pending(storage.as_ref()).await.is_empty());
            assert_eq!(control.protected_reads.load(Ordering::SeqCst), 0);
        }
    }
}

#[tokio::test]
async fn invalid_or_ineligible_inputs_never_send() {
    for mode in ["browser", "provider", "expired", "active", "deleted"] {
        let storage = Arc::new(support::TestStorage::new());
        let mut f = Fixture::new(storage.clone(), mode);
        let runtime = runtime();
        match mode {
            "browser" => {
                f.browser = "not-a-browser-proof".into();
            }
            "provider" => {
                f.provider.session.selected_provider = Some("apple".into());
            }
            "expired" => {
                *f.provider.clock.lock().unwrap() = f.provider.session.expires_at;
            }
            _ => {
                let subject = f
                    .provider
                    .issue(
                        provider_contract::properties(&f.email, "Existing"),
                        &code_digest(),
                    )
                    .await;
                if mode == "deleted" {
                    f.provider
                        .store
                        .delete_account(
                            &subject,
                            irongate::store::DeletedIdentityReusePolicy::Never,
                            0,
                        )
                        .await
                        .unwrap();
                }
            }
        }
        let mail = Mail::default();
        let result = begin(
            &f,
            &f.provider.store,
            &runtime,
            &Default::default(),
            &mail,
            "203.0.113.15",
        )
        .await;
        assert!(matches!(
            result,
            Err(Error::InvalidConfirmation | Error::Expired)
        ));
        assert_eq!(mail.len(), 0);
        assert!(pending(storage.as_ref()).await.is_empty());
    }
}

#[tokio::test]
async fn conditional_failure_and_uncertain_creation_do_not_send_or_retry() {
    for mode in ["before", "lost", "condition", "expired_before_send"] {
        let storage = Arc::new(support::TestStorage::new());
        let f = Fixture::new(storage.clone(), mode);
        let mail = Arc::new(Mail::default());
        let (paused, mut gate) = transaction_gate::PausedStorage::new(storage.clone());
        let task = spawn_begin(&f, f.store(paused), mail.clone(), Default::default());
        let transaction = gate.wait().await;
        let (key, value, expiry) = transaction
            .operations
            .iter()
            .find_map(|op| match op {
                TransactOperation::Put {
                    key, value, expiry, ..
                } if key[0] == "oauth:email_confirmation" => {
                    Some((key.clone(), value.clone(), *expiry))
                }
                _ => None,
            })
            .unwrap();
        match mode {
            "before" => transaction.fail_before_dispatch(),
            "lost" => transaction.lose_response(),
            "condition" => {
                storage
                    .set(
                        &key.iter().map(String::as_str).collect::<Vec<_>>(),
                        value.clone(),
                        expiry,
                    )
                    .await
                    .unwrap();
                transaction.commit();
            }
            _ => {
                *f.provider.clock.lock().unwrap() = expiry.unwrap();
                transaction.commit();
            }
        }
        let error = task.await.unwrap().err().expect("begin must fail");
        match mode {
            "condition" => assert!(matches!(error, Error::InvalidConfirmation)),
            "expired_before_send" => assert!(matches!(error, Error::Expired)),
            _ => assert!(matches!(error, Error::StorageUnavailable)),
        }
        assert_eq!(mail.len(), 0);
        let row = storage
            .get(&key.iter().map(String::as_str).collect::<Vec<_>>())
            .await
            .unwrap();
        assert_eq!(row, if mode == "before" { None } else { Some(value) });
        assert!(storage
            .query_prefix(&["oauth:code"])
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn delivery_error_keeps_pending_state_and_does_not_refund_or_retry() {
    let storage = Arc::new(support::TestStorage::new());
    let f = Fixture::new(storage.clone(), "delivery-lost");
    let mail = Mail {
        fail: true,
        ..Default::default()
    };
    let limits = limited(10, 1);
    let result = begin(
        &f,
        &f.provider.store,
        &runtime(),
        &limits,
        &mail,
        "203.0.113.15",
    )
    .await;
    assert!(matches!(result, Err(Error::DeliveryUnavailable)));
    let before = pending(storage.as_ref()).await;
    assert_eq!(before.len(), 1);
    assert_eq!(mail.len(), 1);
    let retry = begin(
        &f,
        &f.provider.store,
        &runtime(),
        &limits,
        &mail,
        "203.0.113.16",
    )
    .await;
    assert!(matches!(
        retry,
        Err(Error::Admission(AuthError::RateLimitExceeded { .. }))
    ));
    assert_eq!(pending(storage.as_ref()).await, before);
    assert_eq!(mail.len(), 1);
}

#[tokio::test]
async fn expiry_during_delivery_never_returns_a_usable_continuation() {
    let storage = Arc::new(support::TestStorage::new());
    let f = Fixture::new(storage.clone(), "paused-delivery");
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mail = Arc::new(Mail {
        gate: Some((entered.clone(), release.clone())),
        ..Default::default()
    });
    let task = spawn_begin(
        &f,
        f.provider.store.clone(),
        mail.clone(),
        Default::default(),
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    let before = pending(storage.as_ref()).await;
    *f.provider.clock.lock().unwrap() =
        serde_json::from_value(before[0].1["expires_at"].clone()).unwrap();
    release.notify_one();
    assert!(matches!(task.await.unwrap(), Err(Error::Expired)));
    assert_eq!(pending(storage.as_ref()).await, before);
    assert_eq!(mail.len(), 1);
}

#[test]
fn storage_errors_are_classified_without_exposing_backend_values() {
    for error in [
        StorageError::DynamoDB("sensitive-marker".into()),
        StorageError::NotFound("sensitive-marker".into()),
        StorageError::TransactionConflict,
    ] {
        let error = Error::from(error);
        assert!(matches!(error, Error::StorageUnavailable));
        assert!(!format!("{error:?} {error}").contains("sensitive-marker"));
    }
    for error in [
        StorageError::ConditionFailed("sensitive-marker".into()),
        StorageError::AlreadyExists("sensitive-marker".into()),
    ] {
        let error = Error::from(error);
        assert!(matches!(error, Error::InvalidConfirmation));
        assert!(!format!("{error:?} {error}").contains("sensitive-marker"));
    }
}
