use super::*;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Default)]
struct Bytes(Arc<Mutex<Vec<u8>>>);
impl Write for Bytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Force partial writes through the same write_all/flush path used in production.
        let count = bytes.len().min(3);
        self.0.lock().unwrap().extend_from_slice(&bytes[..count]);
        std::thread::yield_now();
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Bytes {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
    fn records(&self) -> Vec<Value> {
        self.text()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn sink(&self) -> Arc<dyn AuditSink> {
        Arc::new(WriterSink::new(self.clone()))
    }
}
fn event() -> AuditEvent<'static> {
    AuditEvent::AuthorizationCodeExchanged {
        client_id: "mobile",
        subject: "user_123",
    }
}
fn assert_envelope(record: &Value) {
    assert_eq!(record["schema_version"], 1);
    uuid::Uuid::parse_str(record["id"].as_str().unwrap()).unwrap();
    chrono::DateTime::parse_from_rfc3339(record["timestamp"].as_str().unwrap()).unwrap();
}

#[test]
fn concurrent_partial_writes_preserve_complete_unique_json_records() {
    let output = Bytes::default();
    let fallback = Bytes::default();
    let emitter = AuditEmitter::new(AuditLogMode::CloudWatch, output.sink(), fallback.sink());
    std::thread::scope(|scope| {
        for _ in 0..32 {
            scope.spawn(|| emitter.emit(event()));
        }
    });
    let records = output.records();
    assert_eq!(records.len(), 32);
    assert!(fallback.text().is_empty());
    let mut ids = HashSet::new();
    for record in records {
        assert_envelope(&record);
        assert_eq!(record["record_type"], "auth_audit");
        assert_eq!(record["level"], "INFO");
        assert_eq!(record["event_type"], "authorization_code_exchanged");
        assert_eq!(record["client_id"], "mobile");
        assert_eq!(record["subject"], "user_123");
        assert!(ids.insert(record["id"].as_str().unwrap().to_owned()));
    }
}

struct FailSink(AtomicUsize);
impl AuditSink for FailSink {
    fn write_line(&self, _: &[u8]) -> io::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::other("SENSITIVE-ERROR-token=password"))
    }
}

#[test]
fn output_failure_is_safe_visible_and_never_recurses() {
    let output = Arc::new(FailSink(AtomicUsize::new(0)));
    let fallback = Bytes::default();
    AuditEmitter::new(AuditLogMode::CloudWatch, output.clone(), fallback.sink()).emit(event());
    assert_eq!(output.0.load(Ordering::SeqCst), 1);
    let records = fallback.records();
    assert_eq!(records.len(), 1);
    let diagnostic = &records[0];
    assert_envelope(diagnostic);
    assert_eq!(diagnostic["record_type"], "audit_emit_failed");
    assert_eq!(diagnostic["level"], "ERROR");
    assert_eq!(diagnostic["category"], "output");
    assert_eq!(diagnostic["event_type"], "authorization_code_exchanged");
    assert_eq!(diagnostic.as_object().unwrap().len(), 7);
    assert!(!fallback.text().contains("SENSITIVE"));
    assert!(!fallback.text().contains("user_123"));
    let failed_fallback = Arc::new(FailSink(AtomicUsize::new(0)));
    AuditEmitter::new(
        AuditLogMode::CloudWatch,
        output.clone(),
        failed_fallback.clone(),
    )
    .emit(event());
    assert_eq!(output.0.load(Ordering::SeqCst), 2);
    assert_eq!(failed_fallback.0.load(Ordering::SeqCst), 1);
    AuditEmitter::new(AuditLogMode::None, output.clone(), failed_fallback.clone()).emit(event());
    assert_eq!(output.0.load(Ordering::SeqCst), 2);
    assert_eq!(failed_fallback.0.load(Ordering::SeqCst), 1);
}

#[test]
fn invalid_values_are_rejected_without_payload_leaks_or_truncation() {
    let output = Bytes::default();
    let fallback = Bytes::default();
    let emitter = AuditEmitter::new(AuditLogMode::CloudWatch, output.sink(), fallback.sink());
    for client_id in [
        "secret@example.com".to_owned(),
        "\nSECRET-header".to_owned(),
        "x".repeat(9000),
        String::new(),
    ] {
        emitter.emit(AuditEvent::AuthorizationCodeExchanged {
            client_id: &client_id,
            subject: "user_123",
        });
    }
    emitter.emit(AuditEvent::RefreshTokenReuse {
        client_id: "mobile",
        token_hash: "RAW-REFRESH-TOKEN",
    });
    emitter.emit(AuditEvent::AdminAccountRead {
        actor: "https://secret.invalid/?token=secret",
        subject: "user_123",
    });
    assert!(output.text().is_empty());
    assert_eq!(fallback.records().len(), 6);
    for record in fallback.records() {
        assert_eq!(record["category"], "invalid_payload");
        assert_eq!(record.as_object().unwrap().len(), 7);
    }
    for marker in ["secret", "SECRET", "RAW-REFRESH", "user_123", "xxx"] {
        assert!(!fallback.text().contains(marker));
    }
}

#[test]
fn serialization_and_size_failures_use_the_same_safe_fallback() {
    struct CannotSerialize;
    impl Serialize for CannotSerialize {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("SECRET-SERIALIZATION"))
        }
    }
    let output = Bytes::default();
    let fallback = Bytes::default();
    let emitter = AuditEmitter::new(AuditLogMode::CloudWatch, output.sink(), fallback.sink());
    let id = uuid::Uuid::new_v4().to_string();
    emitter.finish(json_line(&CannotSerialize), event().kind(), &id);
    emitter.finish(json_line(&"SECRET".repeat(2000)), event().kind(), &id);
    assert!(output.text().is_empty());
    let records = fallback.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["category"], "serialization");
    assert_eq!(records[1]["category"], "oversized");
    assert!(!fallback.text().contains("SECRET"));
    assert_eq!(
        json_line(&"x".repeat(MAX_RECORD_BYTES - 4)).unwrap().len(),
        MAX_RECORD_BYTES - 1
    );
    assert_eq!(
        json_line(&"x".repeat(MAX_RECORD_BYTES - 3)),
        Err(EmitFailure::Oversized)
    );
}

struct BrokenWriter {
    calls: usize,
    fail_flush: bool,
}
impl Write for BrokenWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.fail_flush {
            return Ok(bytes.len());
        }
        if self.calls == 1 {
            return Ok(1);
        }
        Err(io::Error::other("SECRET partial write"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("SECRET flush"))
    }
}
#[test]
fn partial_write_and_flush_errors_are_not_reported_as_success() {
    for fail_flush in [true, false] {
        let fallback = Bytes::default();
        let writer = WriterSink::new(BrokenWriter {
            calls: 0,
            fail_flush,
        });
        AuditEmitter::new(AuditLogMode::CloudWatch, Arc::new(writer), fallback.sink())
            .emit(event());
        assert_eq!(fallback.records().len(), 1);
        assert_eq!(fallback.records()[0]["category"], "output");
        assert!(!fallback.text().contains("SECRET"));
    }
}

#[test]
fn production_stream_output_child() {
    if std::env::var_os("IRONGATE_AUDIT_OUTPUT_CHILD").is_none() {
        return;
    }
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("FILTERED-DIAGNOSTIC");
        let emitter = AuditEmitter::production(AuditLogMode::CloudWatch);
        emitter.emit(event());
        emitter.emit(AuditEvent::AuthorizationCodeExchanged {
            client_id: "SECRET@example.com",
            subject: "user_123",
        });
        AuditEmitter::production(AuditLogMode::None).emit(event());
    });
}
#[test]
fn production_stdout_and_independent_stderr_bypass_tracing_filters() {
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "audit::tests::production_stream_output_child",
            "--nocapture",
            "--quiet",
        ])
        .env("IRONGATE_AUDIT_OUTPUT_CHILD", "1")
        .output()
        .unwrap();
    assert!(result.status.success());
    let out = String::from_utf8(result.stdout).unwrap();
    let err = String::from_utf8(result.stderr).unwrap();
    let records: Vec<Value> = out
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    assert_eq!(records.len(), 1);
    assert_envelope(&records[0]);
    assert_eq!(records[0]["event_type"], "authorization_code_exchanged");
    let diagnostic: Value = serde_json::from_str(err.trim()).unwrap();
    assert_eq!(diagnostic["record_type"], "audit_emit_failed");
    assert_eq!(diagnostic["level"], "ERROR");
    assert!(!out.contains("FILTERED-DIAGNOSTIC"));
    assert!(!format!("{out}{err}").contains("SECRET"));
}
