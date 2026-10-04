//! Caller-owned sinks and the failure-isolated `Observer`. No remote
//! telemetry: a sink is a local in-memory buffer or a caller-chosen file.

use super::event::Observation;
use serde_json::{json, Value};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub const SUMMARY_SCHEMA: &str = "semaprax.harness-observation-summary.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SinkError;

pub trait Sink {
    /// Write one JSONL line (no trailing LF). Failure only increments the
    /// observer's dropped count.
    fn write_line(&mut self, line: &str) -> Result<(), SinkError>;
}

/// Bounded in-memory sink.
#[derive(Debug, Default)]
pub struct MemorySink {
    pub lines: Vec<String>,
    pub max_lines: usize,
}

impl MemorySink {
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
        }
    }
}

impl Sink for MemorySink {
    fn write_line(&mut self, line: &str) -> Result<(), SinkError> {
        if self.lines.len() >= self.max_lines {
            return Err(SinkError);
        }
        self.lines.push(line.to_string());
        Ok(())
    }
}

/// Bounded JSONL file at a caller-supplied path (created, never appended to
/// an existing file).
pub struct JsonlFileSink {
    file: File,
    written: usize,
    max_bytes: usize,
}

impl JsonlFileSink {
    pub fn create(path: &Path, max_bytes: usize) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        Ok(Self {
            file,
            written: 0,
            max_bytes,
        })
    }
}

impl Sink for JsonlFileSink {
    fn write_line(&mut self, line: &str) -> Result<(), SinkError> {
        if self.written + line.len() + 1 > self.max_bytes {
            return Err(SinkError);
        }
        writeln!(self.file, "{line}").map_err(|_| SinkError)?;
        self.written += line.len() + 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObserverLimits {
    pub max_events: usize,
}

impl Default for ObserverLimits {
    fn default() -> Self {
        Self { max_events: 10_000 }
    }
}

/// Host-declared traffic coverage. Undeclared means unknown, not complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostTraffic {
    pub observed: u64,
    pub unobserved: u64,
}

/// Optional observability. `record` returns `()` and never fails, panics or
/// blocks the caller's dispatch: overflow, invalid events and sink errors only
/// increase `dropped`.
pub struct Observer {
    sink: Option<Box<dyn Sink>>,
    limits: ObserverLimits,
    events: Vec<Observation>,
    seq: u64,
    dropped: u64,
    host_traffic: Option<HostTraffic>,
}

impl Observer {
    pub fn new(sink: Option<Box<dyn Sink>>, limits: ObserverLimits) -> Self {
        Self {
            sink,
            limits,
            events: Vec::new(),
            seq: 0,
            dropped: 0,
            host_traffic: None,
        }
    }

    pub fn record(&mut self, mut event: Observation) {
        if self.events.len() >= self.limits.max_events || event.validate().is_err() {
            self.dropped += 1;
            return;
        }
        self.seq += 1;
        event.seq = self.seq;
        if let Some(sink) = self.sink.as_mut() {
            if sink.write_line(&event.to_json().to_string()).is_err() {
                self.dropped += 1;
            }
        }
        self.events.push(event);
    }

    pub fn declare_host_traffic(&mut self, traffic: HostTraffic) {
        self.host_traffic = Some(traffic);
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn events(&self) -> &[Observation] {
        &self.events
    }

    pub fn host_traffic(&self) -> Option<HostTraffic> {
        self.host_traffic
    }

    /// Write the trailing summary line so a JSONL trace is self-describing
    /// about drops and declared traffic coverage.
    pub fn finish(&mut self) {
        let line =
            summary_json(self.events.len() as u64, self.dropped, self.host_traffic).to_string();
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.write_line(&line);
        }
    }
}

pub fn summary_json(events: u64, dropped: u64, traffic: Option<HostTraffic>) -> Value {
    json!({
        "schema": SUMMARY_SCHEMA, "events": events, "dropped": dropped,
        "host_traffic": traffic.map(|t| json!({"observed": t.observed, "unobserved": t.unobserved})),
    })
}
