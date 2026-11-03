#[path = "support/rate_contract.rs"]
mod rate_contract;
#[path = "support/rate_control.rs"]
mod rate_control;
mod support;

async fn memory<F, Fut>(test: F)
where
    F: FnOnce(std::sync::Arc<dyn irongate::storage::StorageAdapter>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    test(std::sync::Arc::new(support::TestStorage::new())).await;
}

rate_contract::rate_cases!(memory);

#[tokio::test]
async fn limiter_diagnostics_omit_backend_errors_and_identities() {
    use rate_contract::*;
    use rate_control::{Fault, RateControl};
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Capture {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let output = Capture(Arc::new(Mutex::new(Vec::new())));
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    // This test binary has one capture subscriber, including concurrent contract cases.
    tracing::subscriber::set_global_default(subscriber).expect("capture subscriber");
    for fault in [Fault::Read, Fault::Write] {
        let control = RateControl::new(Arc::new(support::TestStorage::new()), "source:", fault);
        assert!(matches!(
            check(
                control.as_ref(),
                &config(10, 1),
                chrono::Utc::now(),
                IP,
                EMAIL
            )
            .await,
            Err(irongate::error::AuthError::RateLimitUnavailable)
        ));
    }
    let logs = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("rate limit unavailable"));
    assert!(logs.contains("read") && logs.contains("write"));
    for omitted in [
        IP,
        EMAIL,
        "injected",
        &irongate::store::rate_limits::source_identifier(SECRET, Some(IP)),
    ] {
        assert!(!logs.contains(omitted));
    }
}
