//! Append-only step journal in the cache directory. A restart with the same
//! lineage reads it to decide what may run again: read-only steps may; a
//! side-effecting step that began is never replayed, and an unfinished
//! publication is reported as uncertain, never retried.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub step: String,
    /// `begin`, `done`, `refused`, `uncertain`.
    pub state: String,
    pub detail: Value,
}

pub struct Journal {
    path: PathBuf,
    records: Vec<Record>,
}

fn io(path: &Path, e: impl std::fmt::Display) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPD070", format!("journal {}: {e}", path.display()))
}

impl Journal {
    pub fn open(dir: &Path, lineage: &str) -> HarnessResult<Journal> {
        std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        let path = dir.join(format!("{lineage}.journal.jsonl"));
        let mut records = Vec::new();
        if path.exists() {
            let text = std::fs::read_to_string(&path).map_err(|e| io(&path, e))?;
            for (i, line) in text.lines().enumerate() {
                let v: Value = serde_json::from_str(line)
                    .map_err(|e| io(&path, format!("line {}: {e}", i + 1)))?;
                records.push(Record {
                    seq: v["seq"].as_u64().unwrap_or(0),
                    step: v["step"].as_str().unwrap_or("").into(),
                    state: v["state"].as_str().unwrap_or("").into(),
                    detail: v["detail"].clone(),
                });
            }
        }
        Ok(Journal { path, records })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Append one record and flush it to disk before returning.
    pub fn append(&mut self, step: &str, state: &str, detail: Value) -> HarnessResult<()> {
        let seq = self.records.len() as u64 + 1;
        let line = crate::json::canonical(
            &json!({"seq": seq, "step": step, "state": state, "detail": detail}),
        );
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| io(&self.path, e))?;
        f.write_all(line.as_bytes())
            .and_then(|_| f.write_all(b"\n"))
            .and_then(|_| f.sync_all())
            .map_err(|e| io(&self.path, e))?;
        self.records.push(Record {
            seq,
            step: step.into(),
            state: state.into(),
            detail,
        });
        Ok(())
    }

    /// Latest state recorded for `step`.
    pub fn state(&self, step: &str) -> Option<&Record> {
        self.records.iter().rev().find(|r| r.step == step)
    }

    /// True when `step` has a `begin` with no later terminal record.
    pub fn unfinished(&self, step: &str) -> bool {
        matches!(self.state(step), Some(r) if r.state == "begin")
    }
}
