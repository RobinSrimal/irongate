//! Best-effort security audit output. Local success is not a delivery acknowledgement.

use crate::config::audit::AuditLogMode;
use chrono::Utc;
use serde::Serialize;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

mod events;
pub use events::{AuditEvent, RevocationReason};
const MAX_RECORD_BYTES: usize = 8 * 1024;

/// Receives a complete, validated JSON line. Serialize concurrent writes in the implementation.
pub trait AuditSink: Send + Sync {
    fn write_line(&self, line: &[u8]) -> io::Result<()>;
}

/// Synchronous locked writer; tests also exercise the production byte-writing path through this.
pub struct WriterSink<W>(Mutex<W>);
impl<W: Write + Send> WriterSink<W> {
    pub fn new(writer: W) -> Self {
        Self(Mutex::new(writer))
    }
}
impl<W: Write + Send> AuditSink for WriterSink<W> {
    fn write_line(&self, line: &[u8]) -> io::Result<()> {
        let mut writer = self
            .0
            .lock()
            .map_err(|_| io::Error::other("writer unavailable"))?;
        write_complete(&mut *writer, line)
    }
}
enum ProcessOutput {
    Stdout,
    Stderr,
}
impl AuditSink for ProcessOutput {
    fn write_line(&self, line: &[u8]) -> io::Result<()> {
        // Process-wide lock across all partial writes and flush, including other emitter instances.
        match self {
            Self::Stdout => write_complete(&mut io::stdout().lock(), line),
            Self::Stderr => write_complete(&mut io::stderr().lock(), line),
        }
    }
}
fn write_complete(writer: &mut impl Write, line: &[u8]) -> io::Result<()> {
    writer.write_all(line)?;
    writer.flush()
}

#[derive(Clone)]
pub struct AuditEmitter {
    mode: AuditLogMode,
    output: Arc<dyn AuditSink>,
    fallback: Arc<dyn AuditSink>,
}
impl AuditEmitter {
    pub fn production(mode: AuditLogMode) -> Self {
        Self::new(
            mode,
            Arc::new(ProcessOutput::Stdout),
            Arc::new(ProcessOutput::Stderr),
        )
    }
    pub fn new(
        mode: AuditLogMode,
        output: Arc<dyn AuditSink>,
        fallback: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            mode,
            output,
            fallback,
        }
    }
    /// Attempt once before returning. Failure never changes an auth outcome.
    pub fn emit(&self, event: AuditEvent<'_>) {
        if self.mode == AuditLogMode::None {
            return;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let kind = event.kind();
        let result = if event.is_valid() {
            json_line(&Record {
                record_type: "auth_audit",
                schema_version: 1,
                id: &id,
                timestamp: Utc::now().to_rfc3339(),
                level: "INFO",
                event,
            })
        } else {
            Err(EmitFailure::InvalidPayload)
        };
        self.finish(result, kind, &id);
    }
    fn finish(&self, result: Result<Vec<u8>, EmitFailure>, kind: &str, id: &str) {
        let result = result.and_then(|line| {
            self.output
                .write_line(&line)
                .map_err(|_| EmitFailure::Output)
        });
        if let Err(category) = result {
            // No payload/error text. Independent of tracing and the failed sink.
            // Total output failure cannot reliably be observed; never recurse or panic.
            let diagnostic = FailureRecord {
                record_type: "audit_emit_failed",
                schema_version: 1,
                timestamp: Utc::now().to_rfc3339(),
                level: "ERROR",
                event_type: kind,
                id,
                category,
            };
            if let Ok(line) = json_line(&diagnostic) {
                let _ = self.fallback.write_line(&line);
            }
        }
    }
}
#[derive(Serialize)]
struct Record<'a> {
    record_type: &'static str,
    schema_version: u8,
    id: &'a str,
    timestamp: String,
    level: &'static str,
    #[serde(flatten)]
    event: AuditEvent<'a>,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EmitFailure {
    InvalidPayload,
    Serialization,
    Oversized,
    Output,
}
#[derive(Serialize)]
struct FailureRecord<'a> {
    record_type: &'static str,
    schema_version: u8,
    timestamp: String,
    level: &'static str,
    event_type: &'a str,
    id: &'a str,
    category: EmitFailure,
}
fn json_line(value: &impl Serialize) -> Result<Vec<u8>, EmitFailure> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| EmitFailure::Serialization)?;
    if bytes.len() + 1 >= MAX_RECORD_BYTES {
        return Err(EmitFailure::Oversized);
    }
    bytes.push(b'\n');
    Ok(bytes)
}
#[cfg(test)]
mod tests;
