use irongate::audit::{AuditEmitter, WriterSink};
use irongate::config::audit::AuditLogMode;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Capture(pub Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Capture {
    pub fn emitter(&self, mode: AuditLogMode) -> AuditEmitter {
        AuditEmitter::new(
            mode,
            Arc::new(WriterSink::new(self.clone())),
            Arc::new(WriterSink::new(Capture::default())),
        )
    }

    pub fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }

    pub fn records(&self) -> Vec<serde_json::Value> {
        String::from_utf8(self.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

pub fn emitter(mode: AuditLogMode) -> AuditEmitter {
    Capture::default().emitter(mode)
}

pub struct FailWriter;
impl Write for FailWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("SENSITIVE-SINK-ERROR"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn failing(fallback: &Capture) -> AuditEmitter {
    AuditEmitter::new(
        AuditLogMode::CloudWatch,
        Arc::new(WriterSink::new(FailWriter)),
        Arc::new(WriterSink::new(fallback.clone())),
    )
}

pub fn assert_envelope(record: &serde_json::Value, kind: &str) {
    assert_eq!(record["record_type"], "auth_audit");
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["level"], "INFO");
    assert_eq!(record["event_type"], kind);
    uuid::Uuid::parse_str(record["id"].as_str().unwrap()).unwrap();
    chrono::DateTime::parse_from_rfc3339(record["timestamp"].as_str().unwrap()).unwrap();
    for field in [
        "ip", "detail", "password", "email", "headers", "body", "token",
    ] {
        assert!(record.get(field).is_none(), "unexpected field: {field}");
    }
}
